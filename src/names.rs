use lazy_static::lazy_static;
use regex::Regex;
use rustc_hir::def_id::{DefId, LOCAL_CRATE};
use rustc_middle::ty::{List, TyCtxt};

/// Custom naming scheme for MIR bodies, adapted from def_path_debug_str() in TyCtxt
pub(crate) fn def_id_name<'tcx>(tcx: TyCtxt<'tcx>, def_id: DefId) -> String {
    let substs = List::identity_for_item(tcx, def_id);

    let crate_name = if def_id.is_local() {
        format!("{}::", tcx.crate_name(LOCAL_CRATE))
    } else {
        let cstore = tcx.cstore_untracked();

        // 1) We introduce a ! here, to indicate that the element after it has to be deleted
        format!("{}::!", cstore.crate_name(def_id.krate))
    };

    let mut def_path_str = format!(
        "{}{}",
        crate_name,
        tcx.def_path_str_with_substs(def_id, substs)
    );

    // This is a hack
    //      We are removing the crate prefix in the type that is casted to
    //      This prefix is present if the type is from a non-local crate
    //      We do not want to keep it
    lazy_static! {
        static ref REGEX_CRATE_PREFIX: Regex = Regex::new(r"(<.* as )(.*::)(.*>)").unwrap();
    }
    def_path_str = REGEX_CRATE_PREFIX
        .replace_all(&def_path_str, "$1$3")
        .to_string();

    // This is a hack
    // See 1) above
    // If this is a non-local def_id:
    //      We are removing the part of the path that corresponds to the alias name of the extern crate
    //      In this extern crate itself, this part of the path is not present

    lazy_static! {
        static ref REGEX_LOCAL_ALIAS_1: Regex = Regex::new(r"(!)<").unwrap();
    }
    def_path_str = REGEX_LOCAL_ALIAS_1
        .replace_all(&def_path_str, "<")
        .to_string();

    lazy_static! {
        static ref REGEX_LOCAL_ALIAS_2: Regex = Regex::new(r"(![^:]*?::)").unwrap();
    }
    def_path_str = REGEX_LOCAL_ALIAS_2
        .replace_all(&def_path_str, "")
        .to_string();

    // Ocasionally, there is a newline which we do not want to keep
    def_path_str = def_path_str.replace("\n", "");

    def_path_str
}
