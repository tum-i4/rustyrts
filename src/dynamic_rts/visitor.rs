use std::sync::atomic::AtomicPtr;

use rustc_data_structures::sync::Ordering::SeqCst;
use rustc_hir::AttributeMap;
use rustc_middle::mir::interpret::{ConstValue, GlobalAlloc, Scalar};
use rustc_middle::{
    mir::{visit::MutVisitor, Body, Constant, ConstantKind, Location},
    ty::TyCtxt,
};

use crate::callbacks_shared::TEST_MARKER;
use crate::names::def_id_name;

use super::mir_util::{insert_post, insert_pre, insert_trace};

pub struct MirManipulatorVisitor<'tcx> {
    tcx: TyCtxt<'tcx>,
    processed: Option<AtomicPtr<Body<'tcx>>>,
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

        self.processed = Some(AtomicPtr::new(body as *mut Body<'tcx>));

        let attrs = &self.tcx.hir_crate(()).owners[self
            .tcx
            .local_def_id_to_hir_id(def_id.expect_local())
            .owner
            .def_id]
            .as_owner()
            .map_or(AttributeMap::EMPTY, |o| &o.attrs)
            .map;

        let mut found_test_harness = false;
        for (_, list) in attrs.iter() {
            for attr in *list {
                if attr.name_or_empty().to_ident_string() == TEST_MARKER {
                    let def_path_test = &def_path[0..def_path.len() - 13];

                    // IMPORTANT: The order in which insert_post, insert_pre are called is critical here
                    // 1. insert_post 2. insert_pre

                    insert_post(self.tcx, body, def_path_test);
                    insert_pre(self.tcx, body);
                    found_test_harness = true;
                    break;
                }
            }
        }

        if !found_test_harness {
            insert_trace(self.tcx, body, &def_path);
            self.super_body(body);
        }
    }

    fn visit_constant(&mut self, constant: &mut Constant<'tcx>, location: Location) {
        self.super_constant(constant, location);

        if let Some(ref outer) = self.processed {
            let literal = constant.literal;

            // SAFETY: We stored the currently processed body here before
            let body = unsafe { &mut *outer.load(SeqCst) };

            match literal {
                ConstantKind::Unevaluated(content, _ty) => {
                    // This takes care of borrows of e.g. "const var: u64"
                    let def_path = def_id_name(self.tcx, content.def.did);
                    insert_trace(self.tcx, body, &def_path);
                }
                ConstantKind::Val(cons, _ty) => match cons {
                    ConstValue::Scalar(Scalar::Ptr(ptr, _)) => {
                        match self.tcx.global_alloc(ptr.provenance) {
                            GlobalAlloc::Static(def_id) => {
                                // This takes care of borrows of e.g. "static var: u64"
                                let def_path = def_id_name(self.tcx, def_id);
                                insert_trace(self.tcx, body, &def_path);
                            }
                            GlobalAlloc::Function(instance) => {
                                // TODO: I have not yet found out when this is useful, but since there is a defId stored in here, it might be important
                                // Perhaps this refers to extern fns?
                                let def_path = def_id_name(self.tcx, instance.def_id());
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
