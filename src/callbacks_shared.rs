use rustc_hir::{def_id::LOCAL_CRATE, ConstContext};
use rustc_middle::ty::TyCtxt;
use std::{fs::read, path::PathBuf};

use crate::{
    checksums::{get_checksum, Checksums},
    fs_utils::{get_changes_path, get_checksums_path, get_test_path, write_to_file},
    names::def_id_name,
};

const EXCLUDED_CRATES: &[&str] = &["build_script_build"];

pub(crate) const TEST_MARKER: &str = "rustc_test_marker";

pub(crate) fn excluded<'tcx>(tcx: TyCtxt<'tcx>) -> bool {
    EXCLUDED_CRATES
        .iter()
        .any(|krate| *krate == tcx.crate_name(LOCAL_CRATE).as_str())
}

pub(crate) fn run_analysis_shared<'tcx>(tcx: TyCtxt<'tcx>, path_buf: PathBuf) {
    let crate_name = format!("{}", tcx.crate_name(LOCAL_CRATE));
    let crate_id = tcx.sess.local_stable_crate_id().to_u64();

    //##############################################################################################################
    // 2. Determine which functions represent tests and store the names of those nodes on the filesystem

    let mut tests: Vec<String> = Vec::new();
    for def_id in tcx.mir_keys(()) {
        for attr in tcx.get_attrs_unchecked(def_id.to_def_id()) {
            if attr.name_or_empty().to_ident_string() == TEST_MARKER {
                tests.push(def_id_name(tcx, def_id.to_def_id()));
            }
        }
    }

    if tests.len() > 0 {
        write_to_file(tests.join("\n").to_string(), path_buf.clone(), |buf| {
            get_test_path(buf, &crate_name, crate_id)
        });
    }

    //##################################################################################################################
    // 3. Import checksums

    let checksums_path_buf = get_checksums_path(path_buf.clone(), &crate_name, crate_id);

    let maybe_checksums = read(checksums_path_buf);

    let old_checksums = {
        if let Ok(checksums) = maybe_checksums {
            Checksums::from(checksums.as_slice())
        } else {
            Checksums::new()
        }
    };

    //##############################################################################################################
    // 4. Calculate new checksums and names of changed nodes and write this information to filesystem

    let mut new_checksums = Checksums::new();
    let mut changed_nodes = Vec::new();

    for def_id in tcx.mir_keys(()) {
        let has_body = tcx.hir().maybe_body_owned_by(*def_id).is_some();

        if has_body {
            // Apparently optimized_mir() only works in these two cases
            if let Some(ConstContext::ConstFn) | None = tcx.hir().body_const_context(*def_id) {
                let body = tcx.optimized_mir(*def_id); // See comment above

                //##########################################################################################################
                // Check if checksum changed

                let name = tcx.def_path_debug_str(def_id.to_def_id());
                let checksum = get_checksum(tcx, body);

                let maybe_old = old_checksums.inner().get(&name);

                let changed = match maybe_old {
                    Some(before) => *before != checksum,
                    None => true,
                };

                if changed {
                    changed_nodes.push(def_id_name(tcx, def_id.to_def_id()));
                }
                new_checksums.inner_mut().insert(name, checksum);
            }
        }
    }

    write_to_file(
        new_checksums.to_string().to_string(),
        path_buf.clone(),
        |buf| get_checksums_path(buf, &crate_name, crate_id),
    );

    write_to_file(
        changed_nodes.join("\n").to_string(),
        path_buf.clone(),
        |buf| get_changes_path(buf, &crate_name, crate_id),
    );
}
