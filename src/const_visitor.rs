use std::collections::HashSet;

use rustc_hir::definitions::DefPathData;
use rustc_middle::{
    mir::{interpret::ConstAllocation, ConstValue},
    ty::{EarlyBinder, ParamEnv},
};
use rustc_middle::{
    mir::{
        interpret::{GlobalAlloc, Scalar},
        ConstOperand,
    },
    ty::{Instance, TyKind},
};
use rustc_middle::{
    mir::{visit::TyContext, Const},
    ty::{GenericArg, List, ScalarInt, Ty},
};
use rustc_middle::{
    mir::{visit::Visitor, Body, Location},
    ty::TyCtxt,
};
use rustc_span::def_id::DefId;

use crate::checksums::{get_checksum_const_allocation, get_checksum_scalar_int};

pub struct ResolvingConstVisitor<'tcx> {
    tcx: TyCtxt<'tcx>,
    param_env: ParamEnv<'tcx>,
    acc: HashSet<(u64, u64)>,
    visited: HashSet<(DefId, &'tcx List<GenericArg<'tcx>>)>,
    substs: &'tcx List<GenericArg<'tcx>>,
    processed: Option<DefId>,
}

impl<'tcx> ResolvingConstVisitor<'tcx> {
    pub(crate) fn find_consts(tcx: TyCtxt<'tcx>, body: &'tcx Body<'tcx>) -> HashSet<(u64, u64)> {
        let def_id = body.source.def_id();
        let param_env = tcx.param_env(def_id).with_reveal_all_normalized(tcx);
        let mut resolver = ResolvingConstVisitor {
            tcx,
            param_env,
            acc: HashSet::new(),
            visited: HashSet::new(),
            substs: List::identity_for_item(tcx, def_id),
            processed: None,
        };

        resolver.visit_body(body);
        for body in tcx.promoted_mir(def_id) {
            resolver.visit_body(body);
        }
        resolver.acc
    }

    fn visit(&mut self, def_id: DefId, substs: &'tcx List<GenericArg<'tcx>>) {
        if self.visited.insert((def_id, substs)) && self.tcx.is_mir_available(def_id) {
            let old_processed = self.processed;
            self.processed = Some(def_id);

            let old_substs = self.substs;
            self.substs = substs;

            let body = self.tcx.optimized_mir(def_id);
            self.visit_body(body);
            for body in self.tcx.promoted_mir(def_id) {
                self.visit_body(body);
            }

            self.substs = old_substs;
            self.processed = old_processed;
        }
    }

    fn maybe_const_alloc_from_const_value(
        &self,
        value: ConstValue<'tcx>,
    ) -> Option<Result<ConstAllocation<'tcx>, ScalarInt>> {
        // Result used as either this or that
        match value {
            ConstValue::Scalar(scalar) => match scalar {
                Scalar::Ptr(ptr, _) => {
                    let global_alloc = self.tcx.global_alloc(ptr.provenance.alloc_id());
                    match global_alloc {
                        GlobalAlloc::Static(def_id) => {
                            // If the def path contains a foreign mod, it cannot be computed at compile time
                            let def_path = self.tcx.def_path(def_id);
                            if def_path
                                .data
                                .iter()
                                .any(|d| d.data == DefPathData::ForeignMod)
                            {
                                return None;
                            }

                            self.tcx.eval_static_initializer(def_id).ok().map(Ok)
                        }
                        GlobalAlloc::Memory(const_alloc) => Some(Ok(const_alloc)),
                        _ => None,
                    }
                }
                Scalar::Int(scalar_int) => Some(Err(scalar_int)),
            },
            ConstValue::Slice {
                data: allocation,
                meta: _,
            } => Some(Ok(allocation)),
            ConstValue::Indirect {
                alloc_id: allocation,
                offset: _,
            } => {
                // TODO: check this
                let global_alloc = self.tcx.global_alloc(allocation);
                match global_alloc {
                    GlobalAlloc::Static(def_id) => {
                        // If the def path contains a foreign mod, it cannot be computed at compile time
                        let def_path = self.tcx.def_path(def_id);
                        if def_path
                            .data
                            .iter()
                            .any(|d| d.data == DefPathData::ForeignMod)
                        {
                            return None;
                        }

                        self.tcx.eval_static_initializer(def_id).ok().map(Ok)
                    }
                    GlobalAlloc::Memory(const_alloc) => Some(Ok(const_alloc)),
                    _ => None,
                }
            }
            ConstValue::ZeroSized => None,
        }
    }
}

impl<'tcx> Visitor<'tcx> for ResolvingConstVisitor<'tcx> {
    fn visit_constant(&mut self, constant: &ConstOperand<'tcx>, location: Location) {
        self.super_constant(constant, location);

        let literal = constant.const_;

        let maybe_allocation_or_int = match literal {
            Const::Val(cons, _ty) => self.maybe_const_alloc_from_const_value(cons),
            Const::Unevaluated(unevaluated_cons, _) => {
                let maybe_normalized_cons = self.tcx.try_instantiate_and_normalize_erasing_regions(
                    self.substs,
                    self.param_env,
                    EarlyBinder::bind(unevaluated_cons),
                );

                maybe_normalized_cons.ok().and_then(|unevaluated_cons| {
                    self.tcx
                        .const_eval_resolve(self.param_env, unevaluated_cons, None)
                        .ok()
                        .and_then(|c| self.maybe_const_alloc_from_const_value(c))
                })
            }
            Const::Ty(_) => None,
        };

        if let Some(allocation_or_int) = maybe_allocation_or_int {
            let checksum = match allocation_or_int {
                Ok(allocation) => get_checksum_const_allocation(self.tcx, &allocation),
                Err(scalar_int) => get_checksum_scalar_int(self.tcx, &scalar_int),
            };
            self.acc.insert(checksum);
        }
    }

    fn visit_ty(&mut self, ty: Ty<'tcx>, _ty_context: TyContext) {
        self.super_ty(ty);

        if let Some(outer_def_id) = self.processed {
            match *ty.kind() {
                TyKind::Closure(..) | TyKind::Coroutine(..) | TyKind::FnDef(..) => {
                    // We stop recursing when the function can also be resolved
                    // using the environment of the currently visited function
                    let param_env_outer = self
                        .tcx
                        .param_env(outer_def_id)
                        .with_reveal_all_normalized(self.tcx);

                    let maybe_normalized_ty = match *ty.kind() {
                        TyKind::Closure(..) | TyKind::Coroutine(..) | TyKind::FnDef(..) => self
                            .tcx
                            .try_instantiate_and_normalize_erasing_regions(
                                List::identity_for_item(self.tcx, outer_def_id),
                                param_env_outer,
                                EarlyBinder::bind(ty),
                            )
                            .ok(),
                        _ => None,
                    };

                    if let Some(ty_outer) = maybe_normalized_ty {
                        let (TyKind::Closure(def_id, substs)
                        | TyKind::Coroutine(def_id, substs, _)
                        | TyKind::FnDef(def_id, substs)) = *ty_outer.kind()
                        else {
                            unreachable!()
                        };
                        if let Ok(Some(_)) | Err(_) =
                            Instance::resolve(self.tcx, param_env_outer, def_id, substs)
                        {
                            return;
                        }
                    }
                }
                _ => {}
            }
        }

        let maybe_next = {
            let maybe_normalized_ty = match *ty.kind() {
                TyKind::Closure(..) | TyKind::Coroutine(..) | TyKind::FnDef(..) => self
                    .tcx
                    .try_instantiate_and_normalize_erasing_regions(
                        self.substs,
                        self.param_env,
                        EarlyBinder::bind(ty),
                    )
                    .ok(),
                _ => None,
            };

            maybe_normalized_ty.and_then(|ty| match *ty.kind() {
                TyKind::Closure(def_id, substs)
                | TyKind::Coroutine(def_id, substs, _)
                | TyKind::FnDef(def_id, substs) => {
                    match Instance::resolve(self.tcx, self.param_env, def_id, substs) {
                        Ok(Some(instance)) if !self.tcx.is_closure(instance.def_id()) => {
                            Some((instance.def.def_id(), instance.args))
                        }
                        _ => None,
                    }
                }
                _ => None,
            })
        };

        if let Some((def_id, substs)) = maybe_next {
            self.visit(def_id, substs);
        }
    }
}
