use super::mir_util::Traceable;
use crate::callbacks_shared::TEST_MARKER;
use crate::names::def_id_name;
use log::trace;
use rustc_hir::AttributeMap;
use rustc_middle::{mir::Body, ty::TyCtxt};

pub fn modify_body<'tcx>(tcx: TyCtxt<'tcx>, body: &mut Body<'tcx>) {
    let def_id = body.source.instance.def_id();
    let outer = def_id_name(tcx, def_id, &[], false, true);

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
                let def_path = def_id_name(tcx, def_id, &[], true, false);
                let def_path_test = &def_path[0..def_path.len() - 13];

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
    //let mut visitor = MirInspectingVisitor::new(tcx);
    //visitor.visit_body(&body);
    //let acc = visitor.finalize();

    #[cfg(unix)]
    if outer.ends_with("::main") && body.arg_count == 0 {
        // IMPORTANT: The order in which insert_post, trace, insert_pre are called is critical here
        // 1. insert_post, 2. trace, 3. insert_pre

        body.insert_post_main(tcx, &mut cache_ret, &mut None);
    }

    body.insert_trace(tcx, &outer, &mut cache_str, &mut cache_u8, &mut cache_ret);

    #[cfg(unix)]
    body.check_calls_to_exit(tcx, &mut cache_ret);

    #[cfg(unix)]
    if outer.ends_with("::main") && body.arg_count == 0 {
        body.insert_pre_main(tcx, &mut cache_ret);
    }
}
