use crate::rustc_data_structures::stable_hasher::HashStable;
use regex::Regex;
use rustc_data_structures::stable_hasher::StableHasher;
use rustc_hir::def_id::{DefId, LOCAL_CRATE};
use rustc_interface::Queries;
use rustc_middle::{
    mir::Body,
    ty::{List, TyCtxt},
};
use rustc_session::config::UnstableOptions;

/// Execute a callback function
pub(crate) fn load_tcx<'tcx, F: FnOnce(TyCtxt<'tcx>)>(queries: &'tcx Queries<'tcx>, f: F) {
    queries.global_ctxt().unwrap().enter(|tcx| f(tcx));
}

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
    let regex: Regex = Regex::new(r"(<.* as )(.*::)(.*>)").unwrap();
    def_path_str = regex.replace_all(&def_path_str, "$1$3").to_string();

    // This is a hack
    // See 1) above
    // If this is a non-local def_id:
    //      We are removing the part of the path that corresponds to the alias name of the extern crate
    //      In this extern crate itself, this part of the path is not present
    let regex: Regex = Regex::new(r"(!)<").unwrap();
    def_path_str = regex.replace_all(&def_path_str, "<").to_string();
    let regex: Regex = Regex::new(r"(![^:]*?::)").unwrap();
    def_path_str = regex.replace_all(&def_path_str, "").to_string();

    // Ocasionally, there is a newline which we do not want to keep
    def_path_str = def_path_str.replace("\n", "");

    def_path_str
}

/// Function to obtain a stable checksum of a MIR body
pub(crate) fn get_checksum<'tcx>(tcx: TyCtxt<'tcx>, body: &Body) -> (u64, u64) {
    let incremental_ignore_spans_before: bool;

    // We only temporarily overwrite 'incremental_ignore_spans'
    // We store its old value and restore it later on
    unsafe {
        // SAFETY: We need to forcefully mutate 'incremental_ignore_spans'
        // We only write a boolean value to a boolean attribute
        let u_opts: &mut UnstableOptions = std::mem::transmute(&tcx.sess.opts.unstable_opts);

        incremental_ignore_spans_before = u_opts.incremental_ignore_spans;
        u_opts.incremental_ignore_spans = true;
    }

    let mut hash = (0, 0);
    tcx.with_stable_hashing_context(|ref mut context| {
        // We use the hashing mechanism provided by the compiler to obtain a hash of a MIR body,
        // that is stable beyond the compiler session

        let mut hasher = StableHasher::new();
        body.hash_stable(context, &mut hasher);
        hash = hasher.finalize();
    });

    // We restore the old value of 'incremental_ignore_spans'
    unsafe {
        // SAFETY: We need to forcefully mutate 'incremental_ignore_spans'
        // We only write a boolean value to a boolean attribute
        let u_opts: &mut UnstableOptions = std::mem::transmute(&tcx.sess.opts.unstable_opts);

        u_opts.incremental_ignore_spans = incremental_ignore_spans_before;
    }

    hash
}
