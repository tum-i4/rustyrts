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
use crate::{callbacks_shared::NEW_CHECKSUMS_CONST, checksums::insert_hashmap, names::def_id_name};

pub fn process_consts<'tcx>(tcx: TyCtxt<'tcx>, body: &Body<'tcx>) {
    let def_id = body.source.def_id();
    let param_env = tcx.param_env(def_id).with_reveal_all_normalized(tcx);
    let mut resolver =
        ResolvingConstVisitor::new(tcx, List::identity_for_item(tcx, def_id), param_env);

    //##############################################################################################################
    // Visit body and contained promoted mir

    resolver.visit_body(body);
    for body in tcx.promoted_mir(def_id) {
        resolver.visit_body(body);
    }

    let result = resolver.finalize();

    let name = def_id_name(tcx, def_id, false, true);
    for allocation_or_int in result {
        let checksum = match allocation_or_int {
            Ok(allocation) => get_checksum_const_allocation(tcx, &allocation),
            Err(scalar_int) => {
                let checksum = get_checksum_scalar_int(tcx, &scalar_int);
                checksum
            }
        };

        insert_hashmap(
            &mut *NEW_CHECKSUMS_CONST.get().unwrap().lock().unwrap(),
            &name,
            checksum,
        );
    }
}

struct ResolvingConstVisitor<'tcx> {
    tcx: TyCtxt<'tcx>,
    substs: &'tcx List<GenericArg<'tcx>>,
    param_env: ParamEnv<'tcx>,
    acc: HashSet<Result<ConstAllocation<'tcx>, ScalarInt>>,
    visited: HashSet<DefId>,
    processed: Option<DefId>,
}

impl<'tcx, 'g> ResolvingConstVisitor<'tcx> {
    pub(crate) fn new(
        tcx: TyCtxt<'tcx>,
        substs: &'tcx List<GenericArg<'tcx>>,
        param_env: ParamEnv<'tcx>,
    ) -> ResolvingConstVisitor<'tcx> {
        ResolvingConstVisitor {
            tcx,
            substs,
            param_env,
            acc: HashSet::new(),
            visited: HashSet::new(),
            processed: None,
        }
    }

    pub(crate) fn visit(&mut self, def_id: DefId, substs: &'tcx List<GenericArg<'tcx>>) {
        if self.tcx.is_mir_available(def_id) {
            let body = self.tcx.optimized_mir(def_id);
            let old_processed = self.processed.replace(def_id);
            let old_substs = self.substs;
            self.substs = substs;
            self.visit_body(body);
            self.processed = old_processed;
            self.substs = old_substs;
        }
    }

    pub(crate) fn finalize(self) -> HashSet<Result<ConstAllocation<'tcx>, ScalarInt>> {
        self.acc
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

        if let Some(allocation_or_int) = match literal {
            ConstantKind::Val(cons, _ty) => self.maybe_const_alloc_from_const_value(cons),
            ConstantKind::Unevaluated(mut unevaluated_cons, _) => {
                unevaluated_cons = self.tcx.subst_and_normalize_erasing_regions(
                    self.substs,
                    self.param_env,
                    unevaluated_cons,
                );

                self.tcx
                    .const_eval_resolve(self.param_env, unevaluated_cons, None)
                    .map(|c| self.maybe_const_alloc_from_const_value(c))
                    .unwrap_or(None)
            }
            _ => None,
        } {
            self.acc.insert(allocation_or_int);
        }
    }

    fn visit_ty(&mut self, mut ty: Ty<'tcx>, _ty_context: TyContext) {
        self.super_ty(ty);

        match *ty.kind() {
            TyKind::FnDef(_def_id, _substs) => {
                // We stop recursing when the function can also be resolved
                // using the environment of the currently vistied function
                if let Some(outer_def) = self.processed {
                    let param_env_outer = self
                        .tcx
                        .param_env(outer_def)
                        .with_reveal_all_normalized(self.tcx);

                    let ty_outer = self.tcx.subst_and_normalize_erasing_regions(
                        List::identity_for_item(self.tcx, outer_def),
                        param_env_outer,
                        ty,
                    );

                    let TyKind::FnDef(def_id, substs) = *ty_outer.kind() else {unreachable!()};

                    if let Ok(Some(_)) | Err(_) =
                        Instance::resolve(self.tcx, param_env_outer, def_id, substs)
                    {
                        return;
                    }
                }

                let ref mut visited = self.visited;
                let param_env = self.param_env;

                ty = self
                    .tcx
                    .subst_and_normalize_erasing_regions(self.substs, self.param_env, ty);

                let TyKind::FnDef(def_id, substs) = *ty.kind() else {unreachable!()};

                if let Ok(Some(instance)) = Instance::resolve(self.tcx, param_env, def_id, substs) {
                    match instance.def {
                        InstanceDef::Item(_item) if !self.tcx.is_closure(instance.def_id()) => {
                            let def_id = instance.def_id();
                            let substs = instance.substs;

                            if visited.insert(def_id) {
                                self.visit(def_id, substs);
                            }
                        }
                        _ => {}
                    }
                };
            }
            _ => {}
        };
    }
}
