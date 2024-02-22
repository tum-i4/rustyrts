// Inspired by rustc_monomorphize::collector
// Source: https://doc.rust-lang.org/nightly/nightly-rustc/src/rustc_monomorphize/collector.rs.html
//
// Adapted to extract the dependency relation instead of monomorphization

use hir::{AttributeMap, ConstContext};
use itertools::Itertools;
use log::trace;
use rustc_data_structures::fx::FxHashSet;
use rustc_data_structures::sync::{par_for_each_in, MTLock, MTLockRef};
use rustc_hir as hir;
use rustc_hir::def::DefKind;
use rustc_hir::def_id::{DefId, DefIdMap};
use rustc_hir::lang_items::LangItem;
use rustc_middle::{mir::visit::Visitor as MirVisitor, ty::List};
use rustc_middle::mir::{self, Location};
use rustc_middle::mir::{
    interpret::{AllocId, ErrorHandled, GlobalAlloc, Scalar},
    BasicBlock,
};
use rustc_middle::query::TyCtxtAt;
use rustc_middle::span_bug;
use rustc_middle::ty::adjustment::{CustomCoerceUnsized, PointerCoercion};
use rustc_middle::ty::layout::ValidityRequirement;
use rustc_middle::ty::{
    self, Instance, InstanceDef, Ty, TyCtxt, TypeFoldable, TypeVisitableExt, VtblEntry,
};
use rustc_middle::ty::{GenericArgKind, GenericArgs};
use rustc_middle::{bug, traits};
use rustc_middle::{
    middle::codegen_fn_attrs::CodegenFnAttrFlags, mir::visit::TyContext, ty::GenericParamDefKind,
};
use rustc_middle::{
    mir::{mono::MonoItem, Operand, TerminatorKind},
    ty::TyKind,
};
use rustc_session::{config::EntryFnType, Limit};
use rustc_span::{def_id::LocalDefId, symbol::sym};
use rustc_span::{
    source_map::{dummy_spanned, respan, Spanned},
    ErrorGuaranteed,
};
use rustc_span::{Span, DUMMY_SP};
use std::path::PathBuf;

use crate::{callbacks_shared::TEST_MARKER, names::{mono_def_id_name, def_id_name}, static_rts::graph::EdgeType};

use super::graph::DependencyGraph;

// pub static debug: AtomicBool = AtomicBool::new(false);

#[derive(PartialEq)]
pub enum MonoItemCollectionMode {
    Eager,
    Lazy,
}

#[derive(PartialEq, Debug)]
pub enum MonomorphizationContext {
    Root,
    Local(EdgeType),
    NonLocal(EdgeType),
}

#[derive(Debug)]
pub enum ContextError {
    HasNoEdgeType,
}

impl<'a> TryInto<EdgeType> for &'a MonomorphizationContext {
    type Error = ContextError;

    fn try_into(self) -> Result<EdgeType, Self::Error> {
        match self {
            MonomorphizationContext::Local(edge_type) => Ok(*edge_type),
            MonomorphizationContext::NonLocal(edge_type) => Ok(*edge_type),
            MonomorphizationContext::Root => Err(ContextError::HasNoEdgeType),
        }
    }
}

pub struct CustomUsageMap<'tcx> {
    graph: DependencyGraph<MonoItem<'tcx>>,
    tcx: TyCtxt<'tcx>,
}

type MonoItems<'tcx> = Vec<(Spanned<MonoItem<'tcx>>, MonomorphizationContext)>;

impl<'tcx> CustomUsageMap<'tcx> {
    fn new(tcx: TyCtxt<'tcx>) -> CustomUsageMap<'tcx> {
        CustomUsageMap {
            graph: DependencyGraph::new(),
            tcx,
        }
    }

    fn record_used<'a>(
        &mut self,
        user_item: MonoItem<'tcx>,
        used_items: &[(Spanned<MonoItem<'tcx>>, MonomorphizationContext)],
    ) where
        'tcx: 'a,
    {
        for (used_item, context) in used_items.into_iter() {
            self.graph.add_edge(
                user_item,
                used_item.node,
                context.try_into().unwrap(),
            );
        }
    }

    pub fn finalize(self) -> DependencyGraph<String> {
        self.graph.convert_to_string(self.tcx)
    }
}

pub fn create_dependency_graph<'tcx>(
    tcx: TyCtxt<'tcx>,
    mode: MonoItemCollectionMode,
) -> DependencyGraph<String> {
    let _prof_timer = tcx.prof.generic_activity("dependency_graph_creation");

    let roots = tcx
        .sess
        .time("dependency_graph_creation_root_collections", || {
            collect_roots(tcx, mode)
        });

    trace!("building dependency graph, beginning at roots");

    let mut visited = MTLock::new(FxHashSet::default());
    let mut usage_map = MTLock::new(CustomUsageMap::new(tcx));
    let recursion_limit = tcx.recursion_limit();

    {
        let visited: MTLockRef<'_, _> = &mut visited;
        let usage_map: MTLockRef<'_, _> = &mut usage_map;

        tcx.sess.time("dependency_graph_creation_graph_walk", || {
            par_for_each_in(roots, |root| {
                let mut recursion_depths = DefIdMap::default();
                collect_items_rec(
                    tcx,
                    dummy_spanned(root),
                    visited,
                    &mut recursion_depths,
                    recursion_limit,
                    usage_map,
                );
            });
        });
    }

    let mut graph = usage_map.into_inner().finalize();

    let tests = tcx.sess.time("dependency_graph_root_collection", || {
        collect_test_functions(tcx)
    });

    for test in tests {
        let def_id = test.def_id();
        let name_trimmed = def_id_name(tcx, def_id, false, true);
        let name = mono_def_id_name(tcx, def_id, List::empty(), false, false);
        graph.add_edge(name, name_trimmed, EdgeType::Trimmed);
    }

    graph
}

// Find all non-generic items by walking the HIR. These items serve as roots to
// start monomorphizing from.
fn collect_roots(tcx: TyCtxt<'_>, mode: MonoItemCollectionMode) -> Vec<MonoItem<'_>> {
    trace!("collecting roots");
    let mut roots = Vec::new();

    {
        let entry_fn = tcx.entry_fn(());

        trace!("collect_roots: entry_fn = {:?}", entry_fn);

        let mut collector = RootCollector {
            tcx,
            mode,
            entry_fn,
            output: &mut roots,
        };

        let crate_items = tcx.hir_crate_items(());

        for id in crate_items.items() {
            collector.process_item(id);
        }

        for id in crate_items.impl_items() {
            collector.process_impl_item(id);
        }

        collector.push_extra_entry_roots();
    }

    // We can only codegen items that are instantiable - items all of
    // whose predicates hold. Luckily, items that aren't instantiable
    // can't actually be used, so we can just skip codegenning them.
    roots
        .into_iter()
        .filter_map(
            |(
                Spanned {
                    node: mono_item, ..
                },
                _context,
            )| { mono_item.is_instantiable(tcx).then_some(mono_item) },
        )
        .collect()
}

// Find all test functions. These items serve as roots to start building the dependency graph from.
pub fn collect_test_functions(tcx: TyCtxt<'_>) -> Vec<MonoItem<'_>> {
    trace!("collecting test functions");
    let mut roots = Vec::new();
    {
        for def in tcx.mir_keys(()) {
            let const_context = tcx.hir().body_const_context(*def);
            if let Some(ConstContext::ConstFn) | None = const_context {
                let attrs = &tcx.hir_crate(()).owners
                    [tcx.local_def_id_to_hir_id(*def).owner.def_id]
                    .as_owner()
                    .map_or(AttributeMap::EMPTY, |o| &o.attrs)
                    .map;

                let is_test = attrs
                    .iter()
                    .flat_map(|(_, list)| list.iter())
                    .unique_by(|i| i.id)
                    .any(|attr| attr.name_or_empty().to_ident_string() == TEST_MARKER);

                if is_test {
                    let body = tcx.optimized_mir(def.to_def_id());
                    let maybe_first_bb = body.basic_blocks.get(BasicBlock::from_usize(0));
                    let first_call = maybe_first_bb.and_then(|bb| bb.terminator.as_ref());

                    if let Some(terminator) = first_call {
                        if let TerminatorKind::Call { func, .. } = &terminator.kind {
                            if let Operand::Constant(const_operand) = func {
                                let ty = const_operand.ty();
                                if let TyKind::FnDef(def_id, substs) = ty.kind() {
                                    let instance = Instance::new(*def_id, substs);
                                    let mono_item = MonoItem::Fn(instance);
                                    roots.push(dummy_spanned(mono_item))
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // We can only codegen items that are instantiable - items all of
    // whose predicates hold. Luckily, items that aren't instantiable
    // can't actually be used, so we can just skip codegenning them.
    roots
        .into_iter()
        .filter_map(
            |Spanned {
                 node: mono_item, ..
             }| { mono_item.is_instantiable(tcx).then_some(mono_item) },
        )
        .collect()
}

/// Collect all monomorphized items reachable from `starting_point`
fn collect_items_rec<'tcx>(
    tcx: TyCtxt<'tcx>,
    starting_item: Spanned<MonoItem<'tcx>>,
    visited: MTLockRef<'_, FxHashSet<MonoItem<'tcx>>>,
    recursion_depths: &mut DefIdMap<usize>,
    recursion_limit: Limit,
    usage_map: MTLockRef<'_, CustomUsageMap<'tcx>>,
) {
    if !visited.lock_mut().insert(starting_item.node) {
        // We've been here already, no need to search again.
        return;
    }

    let mut used_items: MonoItems = Vec::new();
    let recursion_depth_reset;

    match starting_item.node {
        MonoItem::Static(def_id) => {
            let instance = Instance::mono(tcx, def_id);

            // Sanity check whether this ended up being collected accidentally
            debug_assert!(should_codegen_locally(tcx, &instance));

            let ty = instance.ty(tcx, ty::ParamEnv::reveal_all());
            visit_drop_use(tcx, ty, true, starting_item.span, &mut used_items);

            recursion_depth_reset = None;

            if let Ok(alloc) = tcx.eval_static_initializer(def_id) {
                for &prov in alloc.inner().provenance().ptrs().values() {
                    collect_alloc(tcx, prov.alloc_id(), &mut used_items);
                }
            }

            if tcx.needs_thread_local_shim(def_id) {
                used_items.push((
                    respan(
                        starting_item.span,
                        MonoItem::Fn(Instance {
                            def: InstanceDef::ThreadLocalShim(def_id),
                            args: GenericArgs::empty(),
                        }),
                    ),
                    // 5.1. function -> accessed static variable
                    MonomorphizationContext::Local(EdgeType::Static),
                ));
            }
        }
        MonoItem::Fn(instance) => {
            // Sanity check whether this ended up being collected accidentally
            debug_assert!(should_codegen_locally(tcx, &instance));

            // Keep track of the monomorphization recursion depth
            recursion_depth_reset = Some(check_recursion_limit(
                tcx,
                instance,
                starting_item.span,
                recursion_depths,
                recursion_limit,
            ));
            check_type_length_limit(tcx, instance);

            rustc_data_structures::stack::ensure_sufficient_stack(|| {
                collect_used_items(tcx, instance, &mut used_items);
            });
        }
        MonoItem::GlobalAsm(item_id) => {
            recursion_depth_reset = None;

            let item = tcx.hir().item(item_id);
            if let hir::ItemKind::GlobalAsm(asm) = item.kind {
                for (op, op_sp) in asm.operands {
                    match op {
                        hir::InlineAsmOperand::Const { .. } => {
                            // Only constants which resolve to a plain integer
                            // are supported. Therefore the value should not
                            // depend on any other items.
                        }
                        hir::InlineAsmOperand::SymFn { anon_const } => {
                            let fn_ty = tcx
                                .typeck_body(anon_const.body)
                                .node_type(anon_const.hir_id);
                            visit_fn_use(tcx, fn_ty, false, *op_sp, &mut used_items, EdgeType::Asm);
                        }
                        hir::InlineAsmOperand::SymStatic { path: _, def_id } => {
                            let instance = Instance::mono(tcx, *def_id);
                            if should_codegen_locally(tcx, &instance) {
                                trace!("collecting static {:?}", def_id);
                                used_items.push((
                                    dummy_spanned(MonoItem::Static(*def_id)),
                                    MonomorphizationContext::Local(EdgeType::Static),
                                ));
                            }
                        }
                        hir::InlineAsmOperand::In { .. }
                        | hir::InlineAsmOperand::Out { .. }
                        | hir::InlineAsmOperand::InOut { .. }
                        | hir::InlineAsmOperand::SplitInOut { .. } => {
                            span_bug!(*op_sp, "invalid operand type for global_asm!")
                        }
                    }
                }
            } else {
                span_bug!(
                    item.span,
                    "Mismatch between hir::Item type and MonoItem type"
                )
            }
        }
    }

    usage_map
        .lock_mut()
        .record_used(starting_item.node, &used_items);

    for (used_item, context) in used_items {
        if let MonomorphizationContext::Local(_) = context {
            collect_items_rec(
                tcx,
                used_item,
                visited,
                recursion_depths,
                recursion_limit,
                usage_map,
            );
        }
    }

    if let Some((def_id, depth)) = recursion_depth_reset {
        recursion_depths.insert(def_id, depth);
    }
}

/// Format instance name that is already known to be too long for rustc.
/// Show only the first 2 types if it is longer than 32 characters to avoid blasting
/// the user's terminal with thousands of lines of type-name.
///
/// If the type name is longer than before+after, it will be written to a file.
fn shrunk_instance_name<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: &Instance<'tcx>,
) -> (String, Option<PathBuf>) {
    let s = instance.to_string();

    // Only use the shrunk version if it's really shorter.
    // This also avoids the case where before and after slices overlap.
    if s.chars().nth(33).is_some() {
        let shrunk = format!("{}", ty::ShortInstance(instance, 4));
        if shrunk == s {
            return (s, None);
        }

        let path = tcx
            .output_filenames(())
            .temp_path_ext("long-type.txt", None);
        let written_to_path = std::fs::write(&path, s).ok().map(|_| path);

        (shrunk, written_to_path)
    } else {
        (s, None)
    }
}

fn check_recursion_limit<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    span: Span,
    recursion_depths: &mut DefIdMap<usize>,
    recursion_limit: Limit,
) -> (DefId, usize) {
    let def_id = instance.def_id();
    let recursion_depth = recursion_depths.get(&def_id).cloned().unwrap_or(0);
    trace!(" => recursion depth={}", recursion_depth);

    let adjusted_recursion_depth = if Some(def_id) == tcx.lang_items().drop_in_place_fn() {
        // HACK: drop_in_place creates tight monomorphization loops. Give
        // it more margin.
        recursion_depth / 4
    } else {
        recursion_depth
    };

    // Code that needs to instantiate the same function recursively
    // more than the recursion limit is assumed to be causing an
    // infinite expansion.
    if !recursion_limit.value_within_limit(adjusted_recursion_depth) {
        let def_span = tcx.def_span(def_id);
        let def_path_str = tcx.def_path_str(def_id);
        let (shrunk, written_to_path) = shrunk_instance_name(tcx, &instance);
        let mut path = PathBuf::new();
        let was_written = if let Some(written_to_path) = written_to_path {
            path = written_to_path;
            Some(())
        } else {
            None
        };
        panic!(
            "Reached recursion limit {:?} {} {:?} {} {:?} {:?}",
            span, shrunk, def_span, def_path_str, was_written, path,
        );
    }

    recursion_depths.insert(def_id, recursion_depth + 1);

    (def_id, recursion_depth)
}

fn check_type_length_limit<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>) {
    let type_length = instance
        .args
        .iter()
        .flat_map(|arg| arg.walk())
        .filter(|arg| match arg.unpack() {
            GenericArgKind::Type(_) | GenericArgKind::Const(_) => true,
            GenericArgKind::Lifetime(_) => false,
        })
        .count();
    trace!(" => type length={}", type_length);

    // Rust code can easily create exponentially-long types using only a
    // polynomial recursion depth. Even with the default recursion
    // depth, you can easily get cases that take >2^60 steps to run,
    // which means that rustc basically hangs.
    //
    // Bail out in these cases to avoid that bad user experience.
    if !tcx.type_length_limit().value_within_limit(type_length) {
        let (shrunk, written_to_path) = shrunk_instance_name(tcx, &instance);
        let span = tcx.def_span(instance.def_id());
        let mut path = PathBuf::new();
        let was_written = if let Some(path2) = written_to_path {
            path = path2;
            Some(())
        } else {
            None
        };
        panic!(
            "Reached type length limit {:?} {} {:?} {:?} {}",
            span, shrunk, was_written, path, type_length,
        );
    }
}

struct MirUsedCollector<'a, 'tcx> {
    tcx: TyCtxt<'tcx>,
    body: &'a mir::Body<'tcx>,
    output: &'a mut MonoItems<'tcx>,
    instance: Instance<'tcx>,
    visiting_call_terminator: bool,
}

impl<'a, 'tcx> MirUsedCollector<'a, 'tcx> {
    pub fn monomorphize<T>(&self, value: T) -> T
    where
        T: TypeFoldable<TyCtxt<'tcx>>,
    {
        trace!("monomorphize: self.instance={:?}", self.instance);
        self.instance.instantiate_mir_and_normalize_erasing_regions(
            self.tcx,
            ty::ParamEnv::reveal_all(),
            ty::EarlyBinder::bind(value),
        )
    }
}

impl<'a, 'tcx> MirVisitor<'tcx> for MirUsedCollector<'a, 'tcx> {
    fn visit_rvalue(&mut self, rvalue: &mir::Rvalue<'tcx>, location: Location) {
        trace!("visiting rvalue {:?}", *rvalue);

        let span = self.body.source_info(location).span;

        match *rvalue {
            // When doing an cast from a regular pointer to a fat pointer, we
            // have to instantiate all methods of the trait being cast to, so we
            // can build the appropriate vtable.
            mir::Rvalue::Cast(
                mir::CastKind::PointerCoercion(PointerCoercion::Unsize),
                ref operand,
                target_ty,
            )
            | mir::Rvalue::Cast(mir::CastKind::DynStar, ref operand, target_ty) => {
                let target_ty = self.monomorphize(target_ty);
                let source_ty = operand.ty(self.body, self.tcx);
                let source_ty = self.monomorphize(source_ty);
                let (source_ty, target_ty) =
                    find_vtable_types_for_unsizing(self.tcx.at(span), source_ty, target_ty);
                // This could also be a different Unsize instruction, like
                // from a fixed sized array to a slice. But we are only
                // interested in things that produce a vtable.
                if (target_ty.is_trait() && !source_ty.is_trait())
                    || (target_ty.is_dyn_star() && !source_ty.is_dyn_star())
                {
                    create_mono_items_for_vtable_methods(
                        self.tcx,
                        target_ty,
                        source_ty,
                        span,
                        self.output,
                    );
                }
            }
            mir::Rvalue::Cast(
                mir::CastKind::PointerCoercion(PointerCoercion::ReifyFnPointer),
                ref operand,
                _,
            ) => {
                let fn_ty = operand.ty(self.body, self.tcx);
                let fn_ty = self.monomorphize(fn_ty);
                visit_fn_use(
                    self.tcx,
                    fn_ty,
                    false,
                    span,
                    self.output,
                    // 6.1. function -> function that is coerced to a function pointer
                    EdgeType::ReifyPtr,
                );
            }
            mir::Rvalue::Cast(
                mir::CastKind::PointerCoercion(PointerCoercion::ClosureFnPointer(_)),
                ref operand,
                _,
            ) => {
                let source_ty = operand.ty(self.body, self.tcx);
                let source_ty = self.monomorphize(source_ty);
                match *source_ty.kind() {
                    ty::Closure(def_id, args) => {
                        let instance = Instance::new(def_id, args);
                        if should_codegen_locally(self.tcx, &instance) {
                            self.output.push((
                                create_fn_mono_item(self.tcx, instance, span),
                                // 6.2. function -> closure that is coerced to a function pointer
                                MonomorphizationContext::Local(EdgeType::ClosurePtr),
                            ));
                        }
                    }
                    _ => bug!(),
                }
            }
            mir::Rvalue::ThreadLocalRef(def_id) => {
                assert!(self.tcx.is_thread_local_static(def_id));
                let instance = Instance::mono(self.tcx, def_id);
                if should_codegen_locally(self.tcx, &instance) {
                    trace!("collecting thread-local static {:?}", def_id);
                    self.output.push((
                        respan(span, MonoItem::Static(def_id)),
                        // 5.1. function -> accessed static variable
                        MonomorphizationContext::Local(EdgeType::Static),
                    ));
                }
            }
            _ => { /* not interesting */ }
        }

        self.super_rvalue(rvalue, location);
    }

    /// This does not walk the constant, as it has been handled entirely here and trying
    /// to walk it would attempt to evaluate the `ty::Const` inside, which doesn't necessarily
    /// work, as some constants cannot be represented in the type system.
    fn visit_constant(&mut self, constant: &mir::ConstOperand<'tcx>, location: Location) {
        let const_ = self.monomorphize(constant.const_);
        let param_env = ty::ParamEnv::reveal_all();
        let val = match const_.eval(self.tcx, param_env, None) {
            Ok(v) => v,
            Err(ErrorHandled::Reported(..)) => return,
            Err(ErrorHandled::TooGeneric(..)) => span_bug!(
                self.body.source_info(location).span,
                "collection encountered polymorphic constant: {:?}",
                const_
            ),
        };
        collect_const_value(self.tcx, val, self.output);
        MirVisitor::visit_ty(self, const_.ty(), TyContext::Location(location));
    }

    fn visit_terminator(&mut self, terminator: &mir::Terminator<'tcx>, location: Location) {
        trace!("visiting terminator {:?} @ {:?}", terminator, location);
        let source = self.body.source_info(location).span;

        let tcx = self.tcx;
        let push_mono_lang_item = |this: &mut Self, lang_item: LangItem| {
            let instance = Instance::mono(tcx, tcx.require_lang_item(lang_item, Some(source)));
            if should_codegen_locally(tcx, &instance) {
                this.output.push((
                    create_fn_mono_item(tcx, instance, source),
                    MonomorphizationContext::Local(EdgeType::LangItem),
                ));
            }
        };

        match terminator.kind {
            mir::TerminatorKind::Call { ref func, .. } => {
                let callee_ty = func.ty(self.body, tcx);
                let callee_ty = self.monomorphize(callee_ty);
                visit_fn_use(
                    self.tcx,
                    callee_ty,
                    true,
                    source,
                    &mut self.output,
                    // 1. function -> callee function (static dispatch)
                    EdgeType::Call,
                )
            }
            mir::TerminatorKind::Drop { ref place, .. } => {
                let ty = place.ty(self.body, self.tcx).ty;
                let ty = self.monomorphize(ty);
                visit_drop_use(self.tcx, ty, true, source, self.output);
            }
            mir::TerminatorKind::InlineAsm { ref operands, .. } => {
                for op in operands {
                    match *op {
                        mir::InlineAsmOperand::SymFn { ref value } => {
                            let fn_ty = self.monomorphize(value.const_.ty());
                            visit_fn_use(
                                self.tcx,
                                fn_ty,
                                false,
                                source,
                                self.output,
                                EdgeType::Asm,
                            );
                        }
                        mir::InlineAsmOperand::SymStatic { def_id } => {
                            let instance = Instance::mono(self.tcx, def_id);
                            if should_codegen_locally(self.tcx, &instance) {
                                trace!("collecting asm sym static {:?}", def_id);
                                self.output.push((
                                    respan(source, MonoItem::Static(def_id)),
                                    MonomorphizationContext::Local(EdgeType::Static),
                                ));
                            }
                        }
                        _ => {}
                    }
                }
            }
            mir::TerminatorKind::Assert { ref msg, .. } => {
                let lang_item = match &**msg {
                    mir::AssertKind::BoundsCheck { .. } => LangItem::PanicBoundsCheck,
                    mir::AssertKind::MisalignedPointerDereference { .. } => {
                        LangItem::PanicMisalignedPointerDereference
                    }
                    _ => LangItem::Panic,
                };
                push_mono_lang_item(self, lang_item);
            }
            mir::TerminatorKind::UnwindTerminate(reason) => {
                push_mono_lang_item(self, reason.lang_item());
            }
            mir::TerminatorKind::Goto { .. }
            | mir::TerminatorKind::SwitchInt { .. }
            | mir::TerminatorKind::UnwindResume
            | mir::TerminatorKind::Return
            | mir::TerminatorKind::Unreachable => {}
            mir::TerminatorKind::CoroutineDrop
            | mir::TerminatorKind::Yield { .. }
            | mir::TerminatorKind::FalseEdge { .. }
            | mir::TerminatorKind::FalseUnwind { .. } => bug!(),
        }

        if let Some(mir::UnwindAction::Terminate(reason)) = terminator.unwind() {
            push_mono_lang_item(self, reason.lang_item());
        }

        self.visiting_call_terminator = matches!(terminator.kind, mir::TerminatorKind::Call { .. });
        self.super_terminator(terminator, location);
        self.visiting_call_terminator = false;
    }

    fn visit_ty(&mut self, ty: Ty<'tcx>, _: TyContext) {
        let source = self.body.span;

        let tcx = self.tcx;

        if let ty::TyKind::Closure(..) | TyKind::Coroutine(..) = ty.kind() {
            let ty = self.monomorphize(ty);
            // 2. function -> contained closure
            visit_fn_use(tcx, ty, false, source, self.output, EdgeType::Contained);
        }
        self.super_ty(ty)
    }

    fn visit_operand(&mut self, operand: &mir::Operand<'tcx>, location: Location) {
        self.super_operand(operand, location);
    }
}

fn visit_drop_use<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
    is_direct_call: bool,
    source: Span,
    output: &mut MonoItems<'tcx>,
) {
    let instance = Instance::resolve_drop_in_place(tcx, ty);
    visit_instance_use(
        tcx,
        instance,
        is_direct_call,
        source,
        output,
        // 4. function -> destructor (`drop()` function) of types that are dropped (manually or automatically)
        EdgeType::Drop,
    );
}

fn visit_fn_use<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
    is_direct_call: bool,
    source: Span,
    output: &mut MonoItems<'tcx>,
    edge_type: EdgeType,
) {
    if let ty::FnDef(def_id, args) = *ty.kind() {
        let instance = if is_direct_call {
            ty::Instance::expect_resolve(tcx, ty::ParamEnv::reveal_all(), def_id, args)
        } else {
            match ty::Instance::resolve_for_fn_ptr(tcx, ty::ParamEnv::reveal_all(), def_id, args) {
                Some(instance) => instance,
                _ => bug!("failed to resolve instance for {ty}"),
            }
        };
        visit_instance_use(tcx, instance, is_direct_call, source, output, edge_type);
    }
    if let ty::Coroutine(def_id, args, _) | ty::Closure(def_id, args) = *ty.kind() {
        let instance = Instance::new(def_id, args);
        visit_instance_use(tcx, instance, false, source, output, edge_type);
    }
}

fn visit_instance_use<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: ty::Instance<'tcx>,
    is_direct_call: bool,
    source: Span,
    output: &mut MonoItems<'tcx>,
    edge_type: EdgeType,
) {
    trace!(
        "visit_item_use({:?}, is_direct_call={:?})",
        instance,
        is_direct_call
    );

    // IMPORTANT: This connects the graphs of multiple crates
    if tcx.is_reachable_non_generic(instance.def_id())
        || instance
            .polymorphize(tcx)
            .upstream_monomorphization(tcx)
            .is_some()
    {
        output.push((
            create_fn_mono_item(tcx, instance, source),
            MonomorphizationContext::NonLocal(edge_type),
        ));
    }
    if let DefKind::Static(_) = tcx.def_kind(instance.def_id()) {
        output.push((
            create_fn_mono_item(tcx, instance, source),
            MonomorphizationContext::NonLocal(edge_type),
        ));
    }

    if !should_codegen_locally(tcx, &instance) {
        return;
    }
    if let ty::InstanceDef::Intrinsic(def_id) = instance.def {
        let name = tcx.item_name(def_id);
        if let Some(_requirement) = ValidityRequirement::from_intrinsic(name) {
            // The intrinsics assert_inhabited, assert_zero_valid, and assert_mem_uninitialized_valid will
            // be lowered in codegen to nothing or a call to panic_nounwind. So if we encounter any
            // of those intrinsics, we need to include a mono item for panic_nounwind, else we may try to
            // codegen a call to that function without generating code for the function itself.
            let def_id = tcx.lang_items().get(LangItem::PanicNounwind).unwrap();
            let panic_instance = Instance::mono(tcx, def_id);
            if should_codegen_locally(tcx, &panic_instance) {
                output.push((
                    create_fn_mono_item(tcx, panic_instance, source),
                    MonomorphizationContext::Local(EdgeType::Intrinsic),
                ));
            }
        } else if tcx.has_attr(def_id, sym::rustc_safe_intrinsic) {
            // Codegen the fallback body of intrinsics with fallback bodies
            let instance = ty::Instance::new(def_id, instance.args);
            if should_codegen_locally(tcx, &instance) {
                output.push((
                    create_fn_mono_item(tcx, instance, source),
                    MonomorphizationContext::Local(EdgeType::Intrinsic),
                ));
            }
        }
    }

    match instance.def {
        ty::InstanceDef::Virtual(..) | ty::InstanceDef::Intrinsic(_) => {
            if !is_direct_call {
                bug!("{:?} being reified", instance);
            }
        }
        ty::InstanceDef::ThreadLocalShim(..) => {
            bug!("{:?} being reified", instance);
        }
        ty::InstanceDef::DropGlue(_, None) => {}
        ty::InstanceDef::DropGlue(_, Some(_)) => {
            // IMPORTANT: We do not want to have an indirection via drop_in_place
            // and instead directly collect all drop functions that are invoked
            let body = tcx.instance_mir(instance.def);
            let terminators = body
                .basic_blocks
                .iter()
                .filter_map(|bb| bb.terminator.as_ref());
            for terminator in terminators {
                match terminator.kind {
                    mir::TerminatorKind::Call { ref func, .. } => {
                        let callee_ty = func.ty(body, tcx);
                        let callee_ty = instance.instantiate_mir_and_normalize_erasing_regions(
                            tcx,
                            ty::ParamEnv::reveal_all(),
                            ty::EarlyBinder::bind(callee_ty),
                        );
                        visit_fn_use(tcx, callee_ty, true, source, output, edge_type)
                    }
                    _ => {}
                }
            }
        }
        ty::InstanceDef::Item(def_id)
            if tcx.is_closure(def_id)
                && (edge_type != EdgeType::FnPtr && edge_type != EdgeType::Contained) => {}
        ty::InstanceDef::VTableShim(..)
        | ty::InstanceDef::ReifyShim(..)
        | ty::InstanceDef::ClosureOnceShim { .. }
        | ty::InstanceDef::Item(..)
        | ty::InstanceDef::FnPtrShim(..)
        | ty::InstanceDef::CloneShim(..)
        | ty::InstanceDef::FnPtrAddrShim(..) => {
            output.push((
                create_fn_mono_item(tcx, instance, source),
                MonomorphizationContext::Local(edge_type),
            ));
        }
    }
}

/// Returns `true` if we should codegen an instance in the local crate, or returns `false` if we
/// can just link to the upstream crate and therefore don't need a mono item.
fn should_codegen_locally<'tcx>(tcx: TyCtxt<'tcx>, instance: &Instance<'tcx>) -> bool {
    let Some(def_id) = instance.def.def_id_if_not_guaranteed_local_codegen() else {
        return true;
    };

    if tcx.is_foreign_item(def_id) {
        // Foreign items are always linked against, there's no way of instantiating them.
        return false;
    }

    if def_id.is_local() {
        // Local items cannot be referred to locally without monomorphizing them locally.
        return true;
    }

    if tcx.is_reachable_non_generic(def_id)
        || instance
            .polymorphize(tcx)
            .upstream_monomorphization(tcx)
            .is_some()
    {
        return false;
    }

    if let DefKind::Static(_) = tcx.def_kind(def_id) {
        // We cannot monomorphize statics from upstream crates.
        return false;
    }

    if !tcx.is_mir_available(def_id) {
        panic!(
            "Unable to find optimized MIR {:?} {}",
            tcx.def_span(def_id),
            tcx.crate_name(def_id.krate),
        );
    }

    true
}

/// For a given pair of source and target type that occur in an unsizing coercion,
/// this function finds the pair of types that determines the vtable linking
/// them.
///
/// For example, the source type might be `&SomeStruct` and the target type
/// might be `&dyn SomeTrait` in a cast like:
///
/// ```rust,ignore (not real code)
/// let src: &SomeStruct = ...;
/// let target = src as &dyn SomeTrait;
/// ```
///
/// Then the output of this function would be (SomeStruct, SomeTrait) since for
/// constructing the `target` fat-pointer we need the vtable for that pair.
///
/// Things can get more complicated though because there's also the case where
/// the unsized type occurs as a field:
///
/// ```rust
/// struct ComplexStruct<T: ?Sized> {
///    a: u32,
///    b: f64,
///    c: T
/// }
/// ```
///
/// In this case, if `T` is sized, `&ComplexStruct<T>` is a thin pointer. If `T`
/// is unsized, `&SomeStruct` is a fat pointer, and the vtable it points to is
/// for the pair of `T` (which is a trait) and the concrete type that `T` was
/// originally coerced from:
///
/// ```rust,ignore (not real code)
/// let src: &ComplexStruct<SomeStruct> = ...;
/// let target = src as &ComplexStruct<dyn SomeTrait>;
/// ```
///
/// Again, we want this `find_vtable_types_for_unsizing()` to provide the pair
/// `(SomeStruct, SomeTrait)`.
///
/// Finally, there is also the case of custom unsizing coercions, e.g., for
/// smart pointers such as `Rc` and `Arc`.
fn find_vtable_types_for_unsizing<'tcx>(
    tcx: TyCtxtAt<'tcx>,
    source_ty: Ty<'tcx>,
    target_ty: Ty<'tcx>,
) -> (Ty<'tcx>, Ty<'tcx>) {
    let ptr_vtable = |inner_source: Ty<'tcx>, inner_target: Ty<'tcx>| {
        let param_env = ty::ParamEnv::reveal_all();
        let type_has_metadata = |ty: Ty<'tcx>| -> bool {
            if ty.is_sized(tcx.tcx, param_env) {
                return false;
            }
            let tail = tcx.struct_tail_erasing_lifetimes(ty, param_env);
            match tail.kind() {
                ty::Foreign(..) => false,
                ty::Str | ty::Slice(..) | ty::Dynamic(..) => true,
                _ => bug!("unexpected unsized tail: {:?}", tail),
            }
        };
        if type_has_metadata(inner_source) {
            (inner_source, inner_target)
        } else {
            tcx.struct_lockstep_tails_erasing_lifetimes(inner_source, inner_target, param_env)
        }
    };

    match (&source_ty.kind(), &target_ty.kind()) {
        (&ty::Ref(_, a, _), &ty::Ref(_, b, _) | &ty::RawPtr(ty::TypeAndMut { ty: b, .. }))
        | (&ty::RawPtr(ty::TypeAndMut { ty: a, .. }), &ty::RawPtr(ty::TypeAndMut { ty: b, .. })) => {
            ptr_vtable(*a, *b)
        }
        (&ty::Adt(def_a, _), &ty::Adt(def_b, _)) if def_a.is_box() && def_b.is_box() => {
            ptr_vtable(source_ty.boxed_ty(), target_ty.boxed_ty())
        }

        // T as dyn* Trait
        (_, &ty::Dynamic(_, _, ty::DynStar)) => ptr_vtable(source_ty, target_ty),

        (&ty::Adt(source_adt_def, source_args), &ty::Adt(target_adt_def, target_args)) => {
            assert_eq!(source_adt_def, target_adt_def);

            let CustomCoerceUnsized::Struct(coerce_index) =
                match custom_coerce_unsize_info(tcx, source_ty, target_ty) {
                    Ok(ccu) => ccu,
                    Err(e) => {
                        let e = Ty::new_error(tcx.tcx, e);
                        return (e, e);
                    }
                };

            let source_fields = &source_adt_def.non_enum_variant().fields;
            let target_fields = &target_adt_def.non_enum_variant().fields;

            assert!(
                coerce_index.index() < source_fields.len()
                    && source_fields.len() == target_fields.len()
            );

            find_vtable_types_for_unsizing(
                tcx,
                source_fields[coerce_index].ty(*tcx, source_args),
                target_fields[coerce_index].ty(*tcx, target_args),
            )
        }
        _ => bug!(
            "find_vtable_types_for_unsizing: invalid coercion {:?} -> {:?}",
            source_ty,
            target_ty
        ),
    }
}

fn create_fn_mono_item<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    source: Span,
) -> Spanned<MonoItem<'tcx>> {
    respan(source, MonoItem::Fn(instance.polymorphize(tcx)))
}

/// Creates a `MonoItem` for each method that is referenced by the vtable for
/// the given trait/impl pair.
fn create_mono_items_for_vtable_methods<'tcx>(
    tcx: TyCtxt<'tcx>,
    trait_ty: Ty<'tcx>,
    impl_ty: Ty<'tcx>,
    source: Span,
    output: &mut MonoItems<'tcx>,
) {
    assert!(!trait_ty.has_escaping_bound_vars() && !impl_ty.has_escaping_bound_vars());

    if let ty::Dynamic(trait_ty, ..) = trait_ty.kind() {
        if let Some(principal) = trait_ty.principal() {
            let poly_trait_ref = principal.with_self_ty(tcx, impl_ty);
            assert!(!poly_trait_ref.has_escaping_bound_vars());

            // Walk all methods of the trait, including those of its supertraits
            let entries = tcx.vtable_entries(poly_trait_ref);
            let methods = entries
                .iter()
                .filter_map(|entry| match entry {
                    VtblEntry::MetadataDropInPlace
                    | VtblEntry::MetadataSize
                    | VtblEntry::MetadataAlign
                    | VtblEntry::Vacant => None,
                    VtblEntry::TraitVPtr(_) => {
                        // all super trait items already covered, so skip them.
                        None
                    }
                    VtblEntry::Method(instance) => {
                        Some(*instance).filter(|instance| should_codegen_locally(tcx, instance))
                    }
                })
                .map(|item| {
                    (
                        create_fn_mono_item(tcx, item, source),
                        // 2.1 function -> function in the vtable of a type that is converted into a dynamic trait object (unsized coercion)
                        // 2.1 function -> function in the vtable of a type that is converted into a dynamic trait object (unsized coercion) + !dyn
                        MonomorphizationContext::Local(EdgeType::Unsize),
                    )
                });
            output.extend(methods);
        }

        // Also add the destructor.
        visit_drop_use(tcx, impl_ty, false, source, output);
    }
}

/// Scans the MIR in order to find function calls, closures, and drop-glue.
fn collect_used_items<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    output: &mut MonoItems<'tcx>,
) {
    let body = tcx.instance_mir(instance.def);

    // Here we rely on the visitor also visiting `required_consts`, so that we evaluate them
    // and abort compilation if any of them errors.
    MirUsedCollector {
        tcx,
        body,
        output,
        instance,
        visiting_call_terminator: false,
    }
    .visit_body(body);
}

fn collect_const_value<'tcx>(
    tcx: TyCtxt<'tcx>,
    value: mir::ConstValue<'tcx>,
    output: &mut MonoItems<'tcx>,
) {
    match value {
        mir::ConstValue::Scalar(Scalar::Ptr(ptr, _size)) => {
            collect_alloc(tcx, ptr.provenance.alloc_id(), output)
        }
        mir::ConstValue::Indirect { alloc_id, .. } => collect_alloc(tcx, alloc_id, output),
        mir::ConstValue::Slice { data, meta: _ } => {
            for &prov in data.inner().provenance().ptrs().values() {
                collect_alloc(tcx, prov.alloc_id(), output);
            }
        }
        _ => {}
    }
}

//=-----------------------------------------------------------------------------
// Root Collection
//=-----------------------------------------------------------------------------

struct RootCollector<'a, 'tcx> {
    tcx: TyCtxt<'tcx>,
    mode: MonoItemCollectionMode,
    output: &'a mut MonoItems<'tcx>,
    entry_fn: Option<(DefId, EntryFnType)>,
}

impl<'v> RootCollector<'_, 'v> {
    fn process_item(&mut self, id: hir::ItemId) {
        match self.tcx.def_kind(id.owner_id) {
            DefKind::Enum | DefKind::Struct | DefKind::Union => {
                if self.mode == MonoItemCollectionMode::Eager
                    && self.tcx.generics_of(id.owner_id).count() == 0
                {
                    trace!("RootCollector: ADT drop-glue for `{id:?}`",);

                    let ty = self
                        .tcx
                        .type_of(id.owner_id.to_def_id())
                        .no_bound_vars()
                        .unwrap();
                    visit_drop_use(self.tcx, ty, true, DUMMY_SP, self.output);
                }
            }
            DefKind::GlobalAsm => {
                trace!(
                    "RootCollector: ItemKind::GlobalAsm({})",
                    self.tcx.def_path_str(id.owner_id)
                );
                self.output.push((
                    dummy_spanned(MonoItem::GlobalAsm(id)),
                    MonomorphizationContext::Root,
                ));
            }
            DefKind::Static(..) => {
                let def_id = id.owner_id.to_def_id();
                trace!(
                    "RootCollector: ItemKind::Static({})",
                    self.tcx.def_path_str(def_id)
                );
                self.output.push((
                    dummy_spanned(MonoItem::Static(def_id)),
                    MonomorphizationContext::Root,
                ));
            }
            DefKind::Const => {
                // const items only generate mono items if they are
                // actually used somewhere. Just declaring them is insufficient.

                // but even just declaring them must collect the items they refer to
                if let Ok(val) = self.tcx.const_eval_poly(id.owner_id.to_def_id()) {
                    collect_const_value(self.tcx, val, self.output);
                }
            }
            DefKind::Impl { .. } => {
                if self.mode == MonoItemCollectionMode::Eager {
                    create_mono_items_for_default_impls(self.tcx, id, self.output);
                }
            }
            DefKind::Fn => {
                self.push_if_root(id.owner_id.def_id);
            }
            _ => {}
        }
    }

    fn process_impl_item(&mut self, id: hir::ImplItemId) {
        if matches!(self.tcx.def_kind(id.owner_id), DefKind::AssocFn) {
            self.push_if_root(id.owner_id.def_id);
        }
    }

    fn is_root(&self, def_id: LocalDefId) -> bool {
        !self
            .tcx
            .generics_of(def_id)
            .requires_monomorphization(self.tcx)
            && match self.mode {
                MonoItemCollectionMode::Eager => true,
                MonoItemCollectionMode::Lazy => {
                    self.entry_fn.and_then(|(id, _)| id.as_local()) == Some(def_id)
                        || self.tcx.is_reachable_non_generic(def_id)
                        || self
                            .tcx
                            .codegen_fn_attrs(def_id)
                            .flags
                            .contains(CodegenFnAttrFlags::RUSTC_STD_INTERNAL_SYMBOL)
                }
            }
    }

    /// If `def_id` represents a root, pushes it onto the list of
    /// outputs. (Note that all roots must be monomorphic.)
    fn push_if_root(&mut self, def_id: LocalDefId) {
        if self.is_root(def_id) {
            trace!("found root");

            let instance = Instance::mono(self.tcx, def_id.to_def_id());
            self.output.push((
                create_fn_mono_item(self.tcx, instance, DUMMY_SP),
                MonomorphizationContext::Root,
            ));
        }
    }

    /// As a special case, when/if we encounter the
    /// `main()` function, we also have to generate a
    /// monomorphized copy of the start lang item based on
    /// the return type of `main`. This is not needed when
    /// the user writes their own `start` manually.
    fn push_extra_entry_roots(&mut self) {
        let Some((main_def_id, EntryFnType::Main { .. })) = self.entry_fn else {
            return;
        };

        let Some(start_def_id) = self.tcx.lang_items().start_fn() else {
            panic!("Start lang item not found")
        };
        let main_ret_ty = self
            .tcx
            .fn_sig(main_def_id)
            .no_bound_vars()
            .unwrap()
            .output();

        // Given that `main()` has no arguments,
        // then its return type cannot have
        // late-bound regions, since late-bound
        // regions must appear in the argument
        // listing.
        let main_ret_ty = self.tcx.normalize_erasing_regions(
            ty::ParamEnv::reveal_all(),
            main_ret_ty.no_bound_vars().unwrap(),
        );

        let start_instance = Instance::resolve(
            self.tcx,
            ty::ParamEnv::reveal_all(),
            start_def_id,
            self.tcx.mk_args(&[main_ret_ty.into()]),
        )
        .unwrap()
        .unwrap();

        self.output.push((
            create_fn_mono_item(self.tcx, start_instance, DUMMY_SP),
            MonomorphizationContext::Root,
        ));
    }
}

fn create_mono_items_for_default_impls<'tcx>(
    tcx: TyCtxt<'tcx>,
    item: hir::ItemId,
    output: &mut MonoItems<'tcx>,
) {
    let Some(impl_) = tcx.impl_trait_ref(item.owner_id) else {
        return;
    };

    if matches!(
        tcx.impl_polarity(impl_.skip_binder().def_id),
        ty::ImplPolarity::Negative
    ) {
        return;
    }

    if tcx
        .generics_of(item.owner_id)
        .own_requires_monomorphization()
    {
        return;
    }

    // Lifetimes never affect trait selection, so we are allowed to eagerly
    // instantiate an instance of an impl method if the impl (and method,
    // which we check below) is only parameterized over lifetime. In that case,
    // we use the ReErased, which has no lifetime information associated with
    // it, to validate whether or not the impl is legal to instantiate at all.
    let only_region_params = |param: &ty::GenericParamDef, _: &_| match param.kind {
        GenericParamDefKind::Lifetime => tcx.lifetimes.re_erased.into(),
        GenericParamDefKind::Const {
            is_host_effect: true,
            ..
        } => tcx.consts.true_.into(),
        GenericParamDefKind::Type { .. } | GenericParamDefKind::Const { .. } => {
            unreachable!(
                "`own_requires_monomorphization` check means that \
                we should have no type/const params"
            )
        }
    };
    let impl_args = GenericArgs::for_item(tcx, item.owner_id.to_def_id(), only_region_params);
    let trait_ref = impl_.instantiate(tcx, impl_args);

    // Unlike 'lazy' monomorphization that begins by collecting items transitively
    // called by `main` or other global items, when eagerly monomorphizing impl
    // items, we never actually check that the predicates of this impl are satisfied
    // in a empty reveal-all param env (i.e. with no assumptions).
    //
    // Even though this impl has no type or const generic parameters, because we don't
    // consider higher-ranked predicates such as `for<'a> &'a mut [u8]: Copy` to
    // be trivially false. We must now check that the impl has no impossible-to-satisfy
    // predicates.
    if tcx.subst_and_check_impossible_predicates((item.owner_id.to_def_id(), impl_args)) {
        return;
    }

    let param_env = ty::ParamEnv::reveal_all();
    let trait_ref = tcx.normalize_erasing_regions(param_env, trait_ref);
    let overridden_methods = tcx.impl_item_implementor_ids(item.owner_id);
    for method in tcx.provided_trait_methods(trait_ref.def_id) {
        if overridden_methods.contains_key(&method.def_id) {
            continue;
        }

        if tcx
            .generics_of(method.def_id)
            .own_requires_monomorphization()
        {
            continue;
        }

        // As mentioned above, the method is legal to eagerly instantiate if it
        // only has lifetime generic parameters. This is validated by
        let args = trait_ref
            .args
            .extend_to(tcx, method.def_id, only_region_params);
        let instance = ty::Instance::expect_resolve(tcx, param_env, method.def_id, args);

        let mono_item = create_fn_mono_item(tcx, instance, DUMMY_SP);
        if mono_item.node.is_instantiable(tcx) && should_codegen_locally(tcx, &instance) {
            output.push((mono_item, MonomorphizationContext::Root));
        }
    }
}

/// Scans the CTFE alloc in order to find function calls, closures, and drop-glue.
fn collect_alloc<'tcx>(tcx: TyCtxt<'tcx>, alloc_id: AllocId, output: &mut MonoItems<'tcx>) {
    match tcx.global_alloc(alloc_id) {
        GlobalAlloc::Static(def_id) => {
            assert!(!tcx.is_thread_local_static(def_id));
            let instance = Instance::mono(tcx, def_id);
            if should_codegen_locally(tcx, &instance) {
                trace!("collecting static {:?}", def_id);
                output.push((
                    dummy_spanned(MonoItem::Static(def_id)),
                    // 5.1. function -> accessed static variable
                    // 5.2. static variable -> static variable that is pointed to
                    MonomorphizationContext::Local(EdgeType::Static),
                ));
            }
        }
        GlobalAlloc::Memory(alloc) => {
            trace!("collecting {:?} with {:#?}", alloc_id, alloc);
            for &prov in alloc.inner().provenance().ptrs().values() {
                rustc_data_structures::stack::ensure_sufficient_stack(|| {
                    collect_alloc(tcx, prov.alloc_id(), output);
                });
            }
        }
        GlobalAlloc::Function(fn_instance) => {
            if should_codegen_locally(tcx, &fn_instance) {
                trace!("collecting {:?} with {:#?}", alloc_id, fn_instance);
                output.push((
                    create_fn_mono_item(tcx, fn_instance, DUMMY_SP),
                    // 5.3. static variable -> function that is pointed to 
                    MonomorphizationContext::Local(EdgeType::FnPtr),
                ));
                // IMPORTANT: This ensures that functions referenced in closures contained in Consts are considered
                // For instance this is the case for all test functions
                if fn_instance.args.len() > 0 {
                    let maybe_arg = fn_instance.args.get(0);
                    let maybe_pointee = maybe_arg.and_then(|arg| arg.as_type());

                    if let Some(pointee) = maybe_pointee {
                        trace!("collecting function pointer to {:#?}", pointee);
                        visit_fn_use(tcx, pointee, false, DUMMY_SP, output, EdgeType::FnPtr);
                    }
                }
            }
        }
        GlobalAlloc::VTable(ty, trait_ref) => {
            let alloc_id = tcx.vtable_allocation((ty, trait_ref));
            collect_alloc(tcx, alloc_id, output)
        }
    }
}

// Source: https://doc.rust-lang.org/nightly/nightly-rustc/src/rustc_monomorphize/lib.rs.html#25-46
fn custom_coerce_unsize_info<'tcx>(
    tcx: TyCtxtAt<'tcx>,
    source_ty: Ty<'tcx>,
    target_ty: Ty<'tcx>,
) -> Result<CustomCoerceUnsized, ErrorGuaranteed> {
    let trait_ref = ty::TraitRef::from_lang_item(
        tcx.tcx,
        LangItem::CoerceUnsized,
        tcx.span,
        [source_ty, target_ty],
    );

    match tcx.codegen_select_candidate((ty::ParamEnv::reveal_all(), trait_ref)) {
        Ok(traits::ImplSource::UserDefined(traits::ImplSourceUserDefinedData {
            impl_def_id,
            ..
        })) => Ok(tcx.coerce_unsized_info(impl_def_id).custom_kind.unwrap()),
        impl_source => {
            bug!("invalid `CoerceUnsized` impl_source: {:?}", impl_source);
        }
    }
}
