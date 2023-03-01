use std::mem::transmute;
use std::sync::atomic::AtomicUsize;

use rustc_data_structures::sync::Ordering::SeqCst;
use rustc_hir::AttributeMap;
use rustc_middle::mir::interpret::{ConstValue, GlobalAlloc, Scalar};
use rustc_middle::{
    mir::{visit::MutVisitor, Body, Constant, ConstantKind, Location},
    ty::TyCtxt,
};

use crate::names::def_id_name;

use super::mir_util::{insert_post, insert_pre, insert_trace};

pub struct MirManipulatorVisitor<'tcx> {
    tcx: TyCtxt<'tcx>,
    processed: Option<AtomicUsize>,
}

impl<'tcx> MirManipulatorVisitor<'tcx> {
    pub fn new(tcx: TyCtxt<'tcx>) -> MirManipulatorVisitor<'tcx> {
        Self {
            tcx,
            processed: None,
        }
    }
}

impl<'tcx> MutVisitor<'tcx> for MirManipulatorVisitor<'tcx> {
    fn visit_body(&mut self, body: &mut Body<'tcx>) {
        let def_id = body.source.instance.def_id();
        let def_path = def_id_name(self.tcx, def_id);

        insert_trace(self.tcx, body, &def_path);

        self.processed = Some(AtomicUsize::new(unsafe { transmute(body as &Body<'tcx>) }));

        // TODO: fix this
        self.super_body(body);

        let attrs = &self.tcx.hir_crate(()).owners[self
            .tcx
            .local_def_id_to_hir_id(def_id.expect_local())
            .owner
            .def_id]
            .as_owner()
            .map_or(AttributeMap::EMPTY, |o| &o.attrs)
            .map;

        for (_, list) in attrs.iter() {
            for attr in *list {
                if attr.name_or_empty().to_ident_string() == "rustc_test_marker" {
                    insert_pre(self.tcx, body);

                    let def_path_test = &def_path[0..def_path.len() - 13];
                    insert_post(self.tcx, body, def_path_test);
                    break;
                }
            }
        }
    }

    fn visit_constant(&mut self, constant: &mut Constant<'tcx>, location: Location) {
        self.super_constant(constant, location);

        if let Some(ref outer) = self.processed {
            let literal = constant.literal;

            match literal {
                ConstantKind::Val(cons, _ty) => match cons {
                    ConstValue::Scalar(Scalar::Ptr(ptr, _)) => {
                        match self.tcx.global_alloc(ptr.provenance) {
                            GlobalAlloc::Static(def_id) => {
                                let def_path = def_id_name(self.tcx, def_id);
                                let body = unsafe { transmute(outer.load(SeqCst)) };
                                insert_trace(self.tcx, body, &def_path);
                            }
                            _ => (),
                        }
                    }
                    _ => (),
                },
                _ => (),
            };
        }
    }

    fn tcx<'a>(&'a self) -> TyCtxt<'tcx> {
        self.tcx
    }
}
