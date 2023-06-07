use log::trace;
use rustc_middle::mir::interpret::{ConstValue, GlobalAlloc, Scalar};
use rustc_middle::{
    mir::{visit::Visitor, Body, Constant, ConstantKind, Location},
    ty::TyCtxt,
};
use rustc_span::def_id::DefId;

use crate::{
    callbacks_shared::NEW_CHECKSUMS,
    checksums::{get_checksum_const_allocation, insert_hashmap},
    names::def_id_name,
};

pub(crate) struct ConstVisitor<'tcx> {
    tcx: TyCtxt<'tcx>,
    processed_def_id: Option<DefId>,
}

impl<'tcx> ConstVisitor<'tcx> {
    pub fn new(tcx: TyCtxt<'tcx>) -> ConstVisitor<'tcx> {
        Self {
            tcx,
            processed_def_id: None,
        }
    }

    pub fn visit(&mut self, body: &Body<'tcx>) {
        let def_id = body.source.instance.def_id();

        self.processed_def_id = Some(def_id);

        //##############################################################################################################
        // Visit body and contained promoted mir

        self.super_body(body);
        for body in self.tcx.promoted_mir(def_id) {
            self.super_body(body)
        }

        self.processed_def_id = None;
    }
}

impl<'tcx> Visitor<'tcx> for ConstVisitor<'tcx> {
    fn visit_constant(&mut self, constant: &Constant<'tcx>, location: Location) {
        self.super_constant(constant, location);

        let literal = constant.literal;

        match literal {
            ConstantKind::Val(cons, _ty) => match cons {
                ConstValue::Scalar(Scalar::Ptr(ptr, _)) => {
                    let maybe_const_allocation = match self.tcx.global_alloc(ptr.provenance) {
                        GlobalAlloc::Static(def_id) => {
                            self.tcx.eval_static_initializer(def_id).ok()
                        }
                        GlobalAlloc::Memory(const_alloc) => Some(const_alloc),
                        _ => None,
                    };

                    if let Some(const_alloc) = maybe_const_allocation {
                        let name = def_id_name(self.tcx, self.processed_def_id.unwrap(), &[]);
                        let checksum_alloc = get_checksum_const_allocation(self.tcx, &const_alloc);

                        trace!(
                            "Inserting checksum {:?} of {:?} to {}",
                            checksum_alloc,
                            const_alloc,
                            name
                        );

                        insert_hashmap(
                            &mut *NEW_CHECKSUMS.get().unwrap().lock().unwrap(),
                            &name,
                            checksum_alloc,
                        );
                    }
                }
                _ => (),
            },
            _ => (),
        };
    }
}
