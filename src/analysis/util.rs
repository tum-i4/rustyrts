use rustc_hir::def_id::{DefId, LOCAL_CRATE};
use rustc_middle::ty::TyCtxt;

pub fn def_path_debug_str_custom(tcx: TyCtxt, def_id: DefId) -> String {
    let crate_name = if def_id.is_local() {
        tcx.crate_name(LOCAL_CRATE)
    } else {
        let cstore = &*tcx.cstore_untracked();
        cstore.crate_name(def_id.krate)
    };

    let mut def_path_str = format!(
        "{}{}",
        crate_name,
        tcx.def_path(def_id).to_string_no_crate_verbose()
    );

    if def_path_str.ends_with("#1") {
        def_path_str = def_path_str.strip_suffix("#1").unwrap().to_string();
    }
    def_path_str
}
