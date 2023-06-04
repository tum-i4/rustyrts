use std::collections::HashSet;

use super::mir_util::Traceable;
use crate::callbacks_shared::TEST_MARKER;
use crate::names::def_id_name;
use log::trace;
use rustc_hir::AttributeMap;
use rustc_middle::mir::interpret::{ConstValue, GlobalAlloc, Scalar};

use rustc_middle::{
    mir::{visit::Visitor, Body, Constant, ConstantKind, Location},
    ty::TyCtxt,
};

pub fn modify_body<'tcx>(tcx: TyCtxt<'tcx>, body: &mut Body<'tcx>) {
    let def_id = body.source.instance.def_id();
    let outer = def_id_name(tcx, def_id, &[]);

    trace!("Visiting {}", outer);

    let mut cache_str = None;
    let mut cache_u8 = None;
    let mut cache_ret = None;

    let attrs = &tcx.hir_crate(()).owners[tcx
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
                    tcx,
                    def_path_test,
                    &mut cache_str,
                    &mut cache_ret,
                    &mut None,
                );
                body.insert_pre_test(tcx, &mut cache_ret);
                return;
            }
        }
    }

    // We collect all relevant nodes in a vec, in order to not modify/move elements while visiting them
    let mut visitor = MirInspectingVisitor::new(tcx);
    visitor.visit_body(&body);
    let acc = visitor.finalize();

    #[cfg(unix)]
    if outer.ends_with("::main") && body.arg_count == 0 {
        // IMPORTANT: The order in which insert_post, trace, insert_pre are called is critical here
        // 1. insert_post, 2. trace, 3. insert_pre

        body.insert_post_main(tcx, &mut cache_ret, &mut None);
    }

    for def_path in &acc {
        body.insert_trace(
            tcx,
            &outer,
            def_path,
            &mut cache_str,
            &mut cache_u8,
            &mut cache_ret,
        );
    }

    #[cfg(unix)]
    body.check_calls_to_exit(tcx, &mut cache_ret);

    #[cfg(unix)]
    if outer.ends_with("::main") && body.arg_count == 0 {
        body.insert_pre_main(tcx, &mut cache_ret);
    }
}

struct MirInspectingVisitor<'tcx> {
    tcx: TyCtxt<'tcx>,
    acc: HashSet<String>,
}

impl<'tcx> MirInspectingVisitor<'tcx> {
    pub fn new(tcx: TyCtxt<'tcx>) -> MirInspectingVisitor<'tcx> {
        Self {
            tcx,
            acc: HashSet::new(),
        }
    }

    pub fn finalize(self) -> HashSet<String> {
        self.acc
    }
}

impl<'tcx> Visitor<'tcx> for MirInspectingVisitor<'tcx> {
    fn visit_body(&mut self, body: &Body<'tcx>) {
        let def_id = body.source.instance.def_id();

        self.acc.insert(def_id_name(self.tcx, def_id, &[]));
        self.super_body(body);

        for body in self.tcx.promoted_mir(def_id) {
            self.super_body(body)
        }
    }

    fn visit_constant(&mut self, constant: &Constant<'tcx>, location: Location) {
        self.super_constant(constant, location);

        let literal = constant.literal;

        match literal {
            ConstantKind::Unevaluated(content, _ty) => {
                // This takes care of borrows of e.g. "const var: u64"
                self.acc.insert(def_id_name(self.tcx, content.def.did, &[]));
            }
            ConstantKind::Val(cons, _ty) => match cons {
                ConstValue::Scalar(Scalar::Ptr(ptr, _)) => {
                    match self.tcx.global_alloc(ptr.provenance) {
                        GlobalAlloc::Static(def_id) => {
                            // This takes care of borrows of e.g. "static var: u64"
                            self.acc.insert(def_id_name(self.tcx, def_id, &[]));
                        }
                        GlobalAlloc::Function(instance) => {
                            // TODO: I have not yet found out when this is useful, but since there is a defId stored in here, it might be important
                            // Perhaps this refers to extern fns?
                            self.acc
                                .insert(def_id_name(self.tcx, instance.def_id(), &[]));
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
