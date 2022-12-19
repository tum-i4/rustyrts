use rustc_hir::def_id::{DefId, LOCAL_CRATE};
use rustc_middle::ty::{List, TyCtxt};

pub fn def_path_debug_str_custom<'tcx>(tcx: TyCtxt<'tcx>, def_id: DefId) -> String {
    let substs = List::identity_for_item(tcx, def_id);

    let crate_name = if def_id.is_local() {
        tcx.crate_name(LOCAL_CRATE)
    } else {
        let cstore = tcx.cstore_untracked();
        cstore.crate_name(def_id.krate)
    };

    let def_path_str = format!(
        "{}::{}",
        crate_name,
        tcx.def_path_str_with_substs(def_id, substs)
    );

    def_path_str
}
