use regex::Regex;
use rustc_hir::def_id::{DefId, LOCAL_CRATE};
use rustc_middle::ty::{List, TyCtxt};

/// Custom naming scheme for MIR bodies, adapted from def_path_debug_str() in TyCtxt
pub fn def_path_debug_str_custom<'tcx>(tcx: TyCtxt<'tcx>, def_id: DefId) -> String {
    let substs = List::identity_for_item(tcx, def_id);

    let crate_name = if def_id.is_local() {
        format!("{}::", tcx.crate_name(LOCAL_CRATE))
    } else {
        let cstore = tcx.cstore_untracked();

        // We introduce a ! here, to indicate that the element after it has to be deleted
        format!("{}::!", cstore.crate_name(def_id.krate))
    };

    let mut def_path_str = format!(
        "{}{}",
        crate_name,
        tcx.def_path_str_with_substs(def_id, substs)
    );

    // This is a hack
    // If this is a non-local def_id:
    //      We are removing the part of the path that corresponds to the alias name of the extern crate
    //      In this crate itself, this part of the path is not present
    let regex: Regex = Regex::new(r"(![^:]*::)").unwrap();
    def_path_str = regex.replace_all(&def_path_str, "").to_string();

    def_path_str
}
