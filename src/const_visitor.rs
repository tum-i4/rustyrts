use rustc_hir::definitions::DefPathData;
use rustc_middle::mir::interpret::ConstAllocation;
use rustc_middle::mir::interpret::{ConstValue, GlobalAlloc, Scalar};
use rustc_middle::ty::{GenericArg, List, ScalarInt};
use rustc_middle::{
    mir::{visit::Visitor, Body, Constant, ConstantKind, Location},
    ty::TyCtxt,
};
use rustc_span::def_id::DefId;

use crate::checksums::{get_checksum_const_allocation, get_checksum_scalar_int};
use crate::{callbacks_shared::NEW_CHECKSUMS, checksums::insert_hashmap, names::def_id_name};

pub(crate) struct ConstVisitor<'tcx> {
    tcx: TyCtxt<'tcx>,
    processed_instance: Option<(DefId, &'tcx List<GenericArg<'tcx>>)>,
    original_substs: Option<&'tcx List<GenericArg<'tcx>>>,
}

impl<'tcx> ConstVisitor<'tcx> {
    pub fn new(tcx: TyCtxt<'tcx>) -> ConstVisitor<'tcx> {
        Self {
            tcx,
            processed_instance: None,
            original_substs: None,
        }
    }

    pub fn visit(&mut self, body: &Body<'tcx>, substs: &'tcx List<GenericArg<'tcx>>) {
        let def_id = body.source.instance.def_id();

        self.processed_instance = Some((
            def_id,
            if cfg!(feature = "monomorphize_all") {
                substs
            } else {
                List::empty()
            },
        ));
        self.original_substs = Some(substs);

        //##############################################################################################################
        // Visit body and contained promoted mir

        self.super_body(body);
        for body in self.tcx.promoted_mir(def_id) {
            self.super_body(body)
        }

        self.processed_instance = None;
        self.original_substs = None;
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

impl<'tcx> Visitor<'tcx> for ConstVisitor<'tcx> {
    fn visit_constant(&mut self, constant: &Constant<'tcx>, location: Location) {
        self.super_constant(constant, location);

        let (def_id, substs) = self.processed_instance.unwrap();
        let literal = constant.literal;

        if let Some(allocation_or_int) = match literal {
            ConstantKind::Val(cons, _ty) => self.maybe_const_alloc_from_const_value(cons),
            ConstantKind::Unevaluated(mut unevaluated_cons, _) => {
                let param_env = self
                    .tcx
                    .param_env(def_id)
                    .with_reveal_all_normalized(self.tcx);

                unevaluated_cons = self.tcx.subst_and_normalize_erasing_regions(
                    self.original_substs.unwrap(),
                    param_env,
                    unevaluated_cons,
                );

                self.tcx
                    .const_eval_resolve(param_env, unevaluated_cons, None)
                    .map(|c| self.maybe_const_alloc_from_const_value(c))
                    .unwrap_or(None)
            }
            _ => None,
        } {
            let name: String = def_id_name(self.tcx, def_id, substs);
            let checksum = match allocation_or_int {
                Ok(allocation) => get_checksum_const_allocation(self.tcx, &allocation),
                Err(scalar_int) => {
                    let checksum = get_checksum_scalar_int(self.tcx, &scalar_int);
                    checksum
                }
            };

            insert_hashmap(
                &mut *NEW_CHECKSUMS.get().unwrap().lock().unwrap(),
                &name,
                checksum,
            );
        }
    }
}
