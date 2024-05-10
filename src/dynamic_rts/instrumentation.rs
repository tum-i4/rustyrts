use super::mir_util::Traceable;
use crate::callbacks_shared::TEST_MARKER;
use crate::names::def_id_name;
use log::trace;
use once_cell::sync::OnceCell;
use rustc_hir::def_id::DefId;
use rustc_hir::AttributeMap;
use rustc_middle::{mir::Body, ty::TyCtxt};

#[cfg(unix)]
static ENTRY_FN: OnceCell<Option<DefId>> = OnceCell::new();

pub(crate) fn modify_body<'tcx>(tcx: TyCtxt<'tcx>, body: &mut Body<'tcx>) {
    let def_id = body.source.instance.def_id();
    let outer = def_id_name(tcx, def_id, false, true);

    trace!("Visiting {}", outer);

    let mut cache_str = None;
    let mut cache_tuple_of_str_and_ptr = None;
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
                let def_path = def_id_name(tcx, def_id, true, false);
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

    #[cfg(unix)]
    if let Some(entry_def) = ENTRY_FN.get_or_init(|| tcx.entry_fn(()).map(|(def_id, _)| def_id)) {
        if def_id == *entry_def {
            // IMPORTANT: The order in which insert_post, trace, insert_pre are called is critical here
            // 1. insert_post, 2. trace, 3. insert_pre

            body.insert_post_main(tcx, &mut cache_ret, &mut None);
        }
    }

    body.insert_trace(tcx, &outer, &mut cache_tuple_of_str_and_ptr, &mut cache_ret);

    #[cfg(unix)]
    body.check_calls_to_exit(tcx, &mut cache_ret);

    #[cfg(unix)]
    if let Some(entry_def) = ENTRY_FN.get().unwrap() {
        if def_id == *entry_def {
            body.insert_pre_main(tcx, &mut cache_ret);
        }
    }
}
