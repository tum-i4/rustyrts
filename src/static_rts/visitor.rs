use std::collections::HashSet;

use crate::constants::SUFFIX_DYN;
use crate::names::def_id_name;
use log::trace;
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
    mir::{Body, Const},
    ty::Instance,
};
use rustc_span::def_id::DefId;

pub(crate) struct ResolvingVisitor<'tcx> {
    tcx: TyCtxt<'tcx>,
    param_env: ParamEnv<'tcx>,
    acc: HashSet<String>,
    visited: HashSet<(DefId, &'tcx List<GenericArg<'tcx>>)>,
    processed: (DefId, &'tcx List<GenericArg<'tcx>>),
}

impl<'tcx, 'g> ResolvingVisitor<'tcx> {
    pub(crate) fn find_dependencies(tcx: TyCtxt<'tcx>, body: &'tcx Body<'tcx>) -> HashSet<String> {
        let def_id = body.source.def_id();
        let param_env = tcx.param_env(def_id).with_reveal_all_normalized(tcx);
        let mut resolver = ResolvingVisitor {
            tcx,
            param_env,
            acc: HashSet::new(),
            visited: HashSet::new(),
            processed: (def_id, List::identity_for_item(tcx, def_id)),
        };

        resolver.visit_body(body);
        for body in tcx.promoted_mir(def_id) {
            resolver.visit_body(body)
        }
        resolver.acc
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
            if let Context::CodeGen(..) = context {
                self.acc.insert(def_id_name(self.tcx, def_id, false, true));
            }
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
                            self.visit(
                                instance.def_id(),
                                instance.args,
                                Context::CodeGen(Dependency::Static),
                            );
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
                TyKind::Adt(adt_def, substs) => self
                    .tcx
                    .adt_destructor(adt_def.did())
                    .map(|destructor| (destructor.did, substs, Dependency::Drop)),
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
                self.acc
                    .insert(def_id_name(self.tcx, def_id, false, true) + SUFFIX_DYN);
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
                            self.visit(*impl_fn, substs, Context::CodeGen(Dependency::Dynamic));
                        }
                    }
                }
            }

            if let Some((def_id, substs, dependency)) = maybe_dependency {
                self.visit(def_id, substs, Context::CodeGen(dependency));
            }
        }
    }
}
