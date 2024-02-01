use std::collections::HashSet;

use crate::constants::SUFFIX_DYN;
use crate::names::def_id_name;
use log::{trace, debug};
use rustc_middle::mir::{
    interpret::{AllocId, GlobalAlloc, Scalar},
    ConstOperand,
};
use rustc_middle::{
    mir::Location,
    ty::{EarlyBinder, GenericArg, InstanceDef, List, Ty, TyCtxt, TyKind},
};
use rustc_middle::{
    mir::{
        visit::{TyContext, Visitor},
        ConstValue,
    },
    ty::ParamEnv,
};
use rustc_middle::{
    mir::Const,
    ty::Instance,
};
use rustc_span::def_id::DefId;

use super::graph::{DependencyGraph, EdgeType};

pub(crate) struct ResolvingVisitor<'tcx> {
    tcx: TyCtxt<'tcx>,
    param_env: ParamEnv<'tcx>,
    visited: HashSet<(DefId, &'tcx List<GenericArg<'tcx>>)>,
    processed: (DefId, &'tcx List<GenericArg<'tcx>>),

    graph: DependencyGraph<String>,
}

impl<'tcx, 'g> ResolvingVisitor<'tcx> {
    pub(crate) fn new(tcx: TyCtxt<'tcx>, main: DefId) -> ResolvingVisitor<'tcx> {
        Self {
            tcx,
            param_env: tcx.param_env_reveal_all_normalized(main),
            visited: HashSet::new(),
            processed: (main, List::identity_for_item(tcx, main)),
            graph: DependencyGraph::new(),
        }
    }

    pub(crate) fn finalize(self) -> DependencyGraph<String> {
        self.graph
    }

    pub(crate) fn register_test(&mut self, def_id: DefId) {
        let trimmed_name = def_id_name(self.tcx, def_id, false, true)
            .trim_end_matches("::{closure#0}")
            .to_string();
        let name = def_id_name(self.tcx, def_id, false, false)
            .trim_end_matches("::{closure#0}")
            .to_string();

        self.graph.add_edge(name, trimmed_name, EdgeType::Trimmed);
        debug!(
            "Registered test {}",
            def_id_name(self.tcx, def_id, false, false)
        );

        let body = self.tcx.optimized_mir(def_id);
        self.processed = (def_id, List::identity_for_item(self.tcx, def_id));

        self.visit_body(body);
        for body in self.tcx.promoted_mir(def_id) {
            self.visit_body(body)
        }

    }

    fn visit(&mut self, def_id: DefId, substs: &'tcx List<GenericArg<'tcx>>, context: Context) {
        if self.visited.insert((def_id, substs)) {
            trace!(
                "Visiting {} - {:?} - {:?} - {:?}",
                def_id_name(self.tcx, def_id, false, true),
                def_id,
                substs,
                context
            );

            let (outer_def_id, _) = self.processed;
            let from = def_id_name(self.tcx, outer_def_id, false, true);
            let to = def_id_name(self.tcx, def_id, false, true);
            self.graph.add_edge(from, to, EdgeType::from(&context));

            if self.tcx.is_mir_available(def_id) {
                let old_processed = self.processed;
                self.processed = (def_id, substs);

                let body = match context {
                    Context::CodeGen(..) => self.tcx.optimized_mir(def_id),
                    Context::Static => self.tcx.mir_for_ctfe(def_id),
                };

                self.visit_body(body);
                for body in self.tcx.promoted_mir(def_id) {
                    self.visit_body(body)
                }
                self.processed = old_processed;
            }
        }
    }
}

#[derive(Debug)]
enum Context {
    CodeGen(Dependency),
    Static,
}

#[derive(Debug)]
enum Dependency {
    Static,
    Dynamic,
    Drop,
    Contained,
}

impl From<&Context> for EdgeType {
    fn from(value: &Context) -> Self {
        match value {
            Context::CodeGen(Dependency::Static) => EdgeType::StaticCall,
            Context::CodeGen(Dependency::Dynamic) => EdgeType::DynamicCall,
            Context::CodeGen(Dependency::Drop) => EdgeType::Drop,
            Context::CodeGen(Dependency::Contained) => EdgeType::Contained,
            Context::Static => EdgeType::Static,
        }
    }
}

impl<'tcx> Visitor<'tcx> for ResolvingVisitor<'tcx> {
    fn visit_constant(&mut self, constant: &ConstOperand<'tcx>, location: Location) {
        self.super_constant(constant, location);

        match constant.const_ {
            Const::Ty(_) => {}
            Const::Unevaluated(..) => {}
            Const::Val(cons, _) => {
                let alloc_ids = match cons {
                    ConstValue::Scalar(Scalar::Ptr(ptr, ..)) => {
                        vec![ptr.provenance.alloc_id()]
                    }
                    ConstValue::Indirect {
                        alloc_id,
                        offset: _,
                    } => {
                        vec![alloc_id]
                    }
                    ConstValue::Slice {
                        data: alloc,
                        meta: _,
                    } => alloc
                        .inner()
                        .provenance()
                        .provenances()
                        .map(|p| p.alloc_id())
                        .collect::<Vec<AllocId>>(),
                    _ => vec![],
                };

                for alloc_id in alloc_ids {
                    match self.tcx.global_alloc(alloc_id) {
                        GlobalAlloc::Function(instance) => {
                            if check_substs(instance.args) {
                                self.visit(
                                    instance.def_id(),
                                    instance.args,
                                    Context::CodeGen(Dependency::Static),
                                );
                            }
                        }
                        GlobalAlloc::Static(def_id) => {
                            self.visit(
                                def_id,
                                List::identity_for_item(self.tcx, def_id),
                                Context::Static,
                            );
                        }
                        _ => {}
                    }
                }
            }
        };
    }

    fn visit_ty(&mut self, ty: Ty<'tcx>, _ty_context: TyContext) {
        self.super_ty(ty);

        let (_outer_def_id, outer_substs) = self.processed;

        if let TyKind::Closure(..) | TyKind::Coroutine(..) | TyKind::Adt(..) | TyKind::FnDef(..) =
            *ty.kind()
        {
            if let TyKind::FnDef(def_id, substs) = ty.kind() {
                let name = def_id_name(self.tcx, *def_id, false, true);
                trace!("Substituting {} {:?} - {:?}", name, substs, outer_substs);
            } else {
                trace!("Substituting {:?} - {:?}", ty, outer_substs);
            }
            let maybe_normalized_ty = self
                .tcx
                .try_instantiate_and_normalize_erasing_regions(
                    outer_substs,
                    self.param_env,
                    EarlyBinder::bind(ty),
                )
                .ok();

            let maybe_dependency = maybe_normalized_ty.and_then(|ty| match *ty.kind() {
                TyKind::Closure(def_id, substs) => Some((def_id, substs, Dependency::Contained)),
                TyKind::Coroutine(def_id, substs, _) => {
                    Some((def_id, substs, Dependency::Contained))
                }
                TyKind::Adt(adt_def, mut substs) => {
                    self.tcx.adt_destructor(adt_def.did()).map(|destructor| {
                        // The Drop impl may have additional type parameters, which we need to incorporate here
                        if let Some(impl_def) = self.tcx.impl_of_method(destructor.did) {
                            substs = substs.rebase_onto(
                                self.tcx,
                                impl_def,
                                List::identity_for_item(self.tcx, impl_def),
                            );
                        }
                        (destructor.did, substs, Dependency::Drop)
                    })
                }
                TyKind::FnDef(def_id, substs) => {
                    match Instance::resolve(self.tcx, self.param_env, def_id, substs) {
                        Ok(Some(instance)) if !self.tcx.is_closure(instance.def_id()) => {
                            match instance.def {
                                InstanceDef::Virtual(def_id, _) => {
                                    Some((def_id, substs, Dependency::Dynamic))
                                }
                                _ => Some((instance.def_id(), instance.args, Dependency::Static)),
                            }
                        }
                        Ok(None) => {
                            trace!("Got Ok(None) for {:?} and {:?}", def_id, substs);
                            None
                        }
                        _ => None,
                    }
                }
                _ => None,
            });

            if let Some((def_id, _substs, Dependency::Dynamic)) = maybe_dependency {
                let name = def_id_name(self.tcx, def_id, false, true);
                self.graph
                    .add_edge(name.clone() + SUFFIX_DYN, name, EdgeType::DynamicCall);

                if let Some(trait_def) = self.tcx.trait_of_item(def_id) {
                    let trait_impls = self.tcx.trait_impls_of(trait_def);

                    let non_blanket_impls = trait_impls
                        .non_blanket_impls()
                        .values()
                        .flat_map(|impls| impls.iter());
                    let blanket_impls = trait_impls.blanket_impls().iter();

                    for impl_def in blanket_impls.chain(non_blanket_impls) {
                        for impl_fn in self.tcx.associated_item_def_ids(impl_def) {
                            let substs = List::identity_for_item(self.tcx, *impl_fn);
                            if check_substs(substs) {
                                self.visit(*impl_fn, substs, Context::CodeGen(Dependency::Dynamic));
                            }
                        }
                    }
                }
            }

            if let Some((def_id, substs, dependency)) = maybe_dependency {
                if let Dependency::Drop = dependency {
                    self.visit(def_id, substs, Context::CodeGen(dependency));
                } else {
                    if check_substs(substs) {
                        self.visit(def_id, substs, Context::CodeGen(dependency));
                    }
                }
            }
        }
    }
}

fn check_substs(substs: &List<GenericArg<'_>>) -> bool {
    !substs.iter().any(|arg| {
        arg.as_type().is_some_and(|ty| {
            if let TyKind::Param(..) = ty.kind() {
                true
            } else {
                false
            }
        })
    })
}
