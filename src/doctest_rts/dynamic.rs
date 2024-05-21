use rustc_middle::ty::TyCtxt;

use tracing::debug;

use crate::{
    callbacks_shared::{CRATE_ID, CRATE_NAME, PATH_BUF_DOCTESTS},
    constants::ENDING_GRAPH,
    fs_utils::{init_path, write_to_file},
    static_rts::visitor::{create_dependency_graph, MonoItemCollectionMode},
};

pub(crate) fn doctests_analysis(tcx: TyCtxt<'_>) {
    if let Some(path_doctests) = PATH_BUF_DOCTESTS.get() {
        let crate_name = CRATE_NAME.get().unwrap();
        let crate_id = *CRATE_ID.get().unwrap();

        let arena = internment::Arena::new();
        let graph = create_dependency_graph(tcx, &arena, MonoItemCollectionMode::Lazy);

        debug!(target = "doctests", "Created graph for {}", crate_name);

        write_to_file(
            graph.to_string(),
            path_doctests.clone(),
            |buf| init_path(buf, crate_name, crate_id, ENDING_GRAPH),
            false,
        );
    }
}
