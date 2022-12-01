use rustc_hir::def_id::DefId;
use rustc_middle::ty::TyCtxt;

pub fn def_path_debug_str_custom(tcx: TyCtxt, def_id: DefId) -> String {
    let mut def_path_str = tcx.def_path_debug_str(def_id);
    if def_path_str.ends_with("#1") {
        def_path_str = def_path_str.strip_suffix("#1").unwrap().to_string();
    }
    def_path_str
}
