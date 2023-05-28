use std::collections::HashSet;

use super::mir_util::Traceable;
use crate::callbacks_shared::TEST_MARKER;
use crate::constants::EDGE_CASE_ALLOCATOR;
use crate::names::def_id_name;
use log::trace;
use rustc_hir::AttributeMap;
use rustc_middle::mir::interpret::{ConstValue, GlobalAlloc, Scalar};

use rustc_middle::{
    mir::{visit::MutVisitor, Body, Constant, ConstantKind, Location},
    ty::TyCtxt,
};

pub struct MirManipulatorVisitor<'tcx> {
    tcx: TyCtxt<'tcx>,
    acc: HashSet<String>,
}

impl<'tcx> MirManipulatorVisitor<'tcx> {
    pub fn new(tcx: TyCtxt<'tcx>) -> MirManipulatorVisitor<'tcx> {
        Self {
            tcx,
            acc: HashSet::new(),
        }
    }
}

impl<'tcx> MutVisitor<'tcx> for MirManipulatorVisitor<'tcx> {
    fn visit_body(&mut self, body: &mut Body<'tcx>) {
        let def_id = body.source.instance.def_id();
        let outer = def_id_name(self.tcx, def_id);

        if EDGE_CASE_ALLOCATOR.iter().any(|c| outer.ends_with(c)) {
            panic!("Dynamic RustyRTS does not support using a custom allocator. Please use static RustyRTS instead.")
        }

        trace!("Visiting {}", outer);

        self.acc.clear();

        let mut cache_str = None;
        let mut cache_u8 = None;
        let mut cache_ret = None;

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
                if attr.name_or_empty().to_ident_string() == TEST_MARKER {
                    let def_path_test = &outer[0..outer.len() - 13];

                    // IMPORTANT: The order in which insert_post, insert_pre are called is critical here
                    // 1. insert_post 2. insert_pre

                    body.insert_post_test(
                        self.tcx,
                        def_path_test,
                        &mut cache_str,
                        &mut cache_ret,
                        &mut None,
                    );
                    body.insert_pre_test(self.tcx, &mut cache_ret);
                    return;
                }
            }
        }

        // We collect all relevant nodes in a vec, in order to not modify/move elements while visiting them
        self.acc.insert(outer.clone());
        self.super_body(body);

        #[cfg(unix)]
        if outer.ends_with("::main") {
            // IMPORTANT: The order in which insert_post, trace, insert_pre are called is critical here
            // 1. insert_post, 2. trace, 3. insert_pre

            body.insert_post_main(self.tcx, &mut cache_ret, &mut None);
        }

        for def_path in &self.acc {
            body.insert_trace(
                self.tcx,
                def_path,
                &mut cache_str,
                &mut cache_u8,
                &mut cache_ret,
            );
        }

        #[cfg(unix)]
        body.check_calls_to_exit(self.tcx, &mut cache_ret);

        #[cfg(unix)]
        if outer.ends_with("::main") {
            body.insert_pre_main(self.tcx, &mut cache_ret);
        }
    }

    fn visit_constant(&mut self, constant: &mut Constant<'tcx>, location: Location) {
        self.super_constant(constant, location);

        let literal = constant.literal;

        match literal {
            ConstantKind::Unevaluated(content, _ty) => {
                // This takes care of borrows of e.g. "const var: u64"
                self.acc.insert(def_id_name(self.tcx, content.def.did));
            }
            ConstantKind::Val(cons, _ty) => match cons {
                ConstValue::Scalar(Scalar::Ptr(ptr, _)) => {
                    match self.tcx.global_alloc(ptr.provenance) {
                        GlobalAlloc::Static(def_id) => {
                            // This takes care of borrows of e.g. "static var: u64"
                            self.acc.insert(def_id_name(self.tcx, def_id));
                        }
                        GlobalAlloc::Function(instance) => {
                            // TODO: I have not yet found out when this is useful, but since there is a defId stored in here, it might be important
                            // Perhaps this refers to extern fns?
                            self.acc.insert(def_id_name(self.tcx, instance.def_id()));
                        }
                        _ => (),
                    }
                }
                _ => (),
            },
            _ => (),
        };
    }

    fn tcx<'a>(&'a self) -> TyCtxt<'tcx> {
        self.tcx
    }
}
