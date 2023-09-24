use std::collections::HashSet;

use rustc_hir::definitions::DefPathData;
use rustc_middle::{mir::interpret::ConstAllocation, ty::ParamEnv};
use rustc_middle::{
    mir::interpret::{ConstValue, GlobalAlloc, Scalar},
    ty::{Instance, InstanceDef, TyKind},
};
use rustc_middle::{
    mir::visit::TyContext,
    ty::{GenericArg, List, ScalarInt, Ty},
};
use rustc_middle::{
    mir::{visit::Visitor, Body, Constant, ConstantKind, Location},
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

impl<'tcx, 'g> ResolvingConstVisitor<'tcx> {
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
            resolver.visit_body(body)
        }
        resolver.acc
    }

    fn visit(&mut self, def_id: DefId, substs: &'tcx List<GenericArg<'tcx>>) {
        if self.visited.insert((def_id, substs)) {
            if self.tcx.is_mir_available(def_id) {
                let old_processed = self.processed;
                self.processed = Some(def_id);

                let old_substs = self.substs;
                self.substs = substs;

                let body = self.tcx.optimized_mir(def_id);
                self.visit_body(body);
                for body in self.tcx.promoted_mir(def_id) {
                    self.visit_body(body)
                }

                self.substs = old_substs;
                self.processed = old_processed;
            }
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
                    let global_alloc = self.tcx.global_alloc(ptr.provenance);
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

                            self.tcx.eval_static_initializer(def_id).ok().map(|s| Ok(s))
                        }
                        GlobalAlloc::Memory(const_alloc) => Some(Ok(const_alloc)),
                        _ => None,
                    }
                }
                Scalar::Int(scalar_int) => Some(Err(scalar_int)),
            },
            ConstValue::Slice {
                data: allocation,
                start: _,
                end: _,
            } => Some(Ok(allocation)),
            ConstValue::ByRef {
                alloc: allocation,
                offset: _,
            } => Some(Ok(allocation)),
            _ => None,
        }
    }
}

impl<'tcx> Visitor<'tcx> for ResolvingConstVisitor<'tcx> {
    fn visit_constant(&mut self, constant: &Constant<'tcx>, location: Location) {
        self.super_constant(constant, location);

        let literal = constant.literal;

        let maybe_allocation_or_int = match literal {
            ConstantKind::Val(cons, _ty) => self.maybe_const_alloc_from_const_value(cons),
            ConstantKind::Unevaluated(unevaluated_cons, _) => {
                let maybe_normalized_cons = self.tcx.try_subst_and_normalize_erasing_regions(
                    self.substs,
                    self.param_env,
                    unevaluated_cons,
                );

                maybe_normalized_cons.ok().and_then(|unevaluated_cons| {
                    self.tcx
                        .const_eval_resolve(self.param_env, unevaluated_cons, None)
                        .ok()
                        .and_then(|c| self.maybe_const_alloc_from_const_value(c))
                })
            }
            _ => None,
        };

        if let Some(allocation_or_int) = maybe_allocation_or_int {
            let checksum = match allocation_or_int {
                Ok(allocation) => get_checksum_const_allocation(self.tcx, &allocation),
                Err(scalar_int) => {
                    let checksum = get_checksum_scalar_int(self.tcx, &scalar_int);
                    checksum
                }
            };
            self.acc.insert(checksum);
        }
    }

    fn visit_ty(&mut self, ty: Ty<'tcx>, _ty_context: TyContext) {
        self.super_ty(ty);

        if let Some(outer_def_id) = self.processed {
            match *ty.kind() {
                TyKind::FnDef(_def_id, _substs) => {
                    // We stop recursing when the function can also be resolved
                    // using the environment of the currently vistied function
                    let param_env_outer = self
                        .tcx
                        .param_env(outer_def_id)
                        .with_reveal_all_normalized(self.tcx);

                    let maybe_normalized_ty = match *ty.kind() {
                        TyKind::FnDef(..) => self
                            .tcx
                            .try_subst_and_normalize_erasing_regions(
                                List::identity_for_item(self.tcx, outer_def_id),
                                param_env_outer,
                                ty,
                            )
                            .ok(),
                        _ => None,
                    };

                    if let Some(ty_outer) = maybe_normalized_ty {
                        let TyKind::FnDef(def_id_normalized_outer, substs_normalized_outer) = *ty_outer.kind() else {unreachable!()};
                        if let Ok(Some(_)) | Err(_) = Instance::resolve(
                            self.tcx,
                            param_env_outer,
                            def_id_normalized_outer,
                            substs_normalized_outer,
                        ) {
                            return;
                        }
                    }
                }
                _ => {}
            }
        }

        let maybe_next = {
            let maybe_normalized_ty = match *ty.kind() {
                TyKind::Closure(..) | TyKind::Generator(..) | TyKind::FnDef(..) => self
                    .tcx
                    .try_subst_and_normalize_erasing_regions(self.substs, self.param_env, ty)
                    .ok(),
                _ => None,
            };

            maybe_normalized_ty.and_then(|ty| match *ty.kind() {
                // TyKind::Closure(def_id, substs) => Some((def_id, substs)),
                // TyKind::Generator(def_id, substs, _) => Some((def_id, substs)),
                TyKind::FnDef(def_id, substs) => {
                    match Instance::resolve(self.tcx, self.param_env, def_id, substs) {
                        Ok(Some(instance)) /*if !self.tcx.is_closure(instance.def_id())*/ => {
                            match instance.def {
                                InstanceDef::Item(item) => {
                                    Some((item.def_id_for_type_of(), instance.substs))
                                }
                                InstanceDef::Virtual(def_id, _)
                                | InstanceDef::ReifyShim(def_id) => Some((def_id, substs)),
                                InstanceDef::FnPtrShim(def_id, ty) => {
                                    self.visit_ty(ty, _ty_context);
                                    Some((def_id, substs))
                                }
                                InstanceDef::DropGlue(def_id, maybe_ty) => {
                                    if let Some(ty) = maybe_ty {
                                        self.visit_ty(ty, _ty_context);
                                    }
                                    Some((def_id, substs))
                                }
                                InstanceDef::CloneShim(def_id, ty) => {
                                    self.visit_ty(ty, _ty_context);
                                    Some((def_id, substs))
                                }

                                InstanceDef::Intrinsic(def_id)
                                | InstanceDef::VTableShim(def_id)
                                | InstanceDef::ClosureOnceShim {
                                    call_once: def_id,
                                    track_caller: _,
                                } => Some((def_id, substs)),
                            }
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
