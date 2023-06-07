use itertools::Itertools;
use log::{debug, trace};
use once_cell::sync::OnceCell;
use rustc_hir::def_id::LOCAL_CRATE;
use rustc_middle::ty::TyCtxt;
use std::{
    collections::HashSet,
    fs::read,
    sync::{atomic::AtomicUsize, Mutex},
};

use crate::{
    checksums::Checksums,
    constants::ENV_TARGET_DIR,
    fs_utils::{
        get_changes_path, get_checksums_path, get_checksums_vtbl_path, get_test_path, write_to_file,
    },
    names::def_id_name,
    static_rts::callback::PATH_BUF,
};

pub(crate) static OLD_VTABLE_ENTRIES: AtomicUsize = AtomicUsize::new(0);

pub(crate) static CRATE_NAME: OnceCell<String> = OnceCell::new();
pub(crate) static CRATE_ID: OnceCell<u64> = OnceCell::new();

pub(crate) static NODES: OnceCell<Mutex<HashSet<String>>> = OnceCell::new();
pub(crate) static NEW_CHECKSUMS: OnceCell<Mutex<Checksums>> = OnceCell::new();
pub(crate) static NEW_CHECKSUMS_VTBL: OnceCell<Mutex<Checksums>> = OnceCell::new();

const EXCLUDED_CRATES: &[&str] = &["build_script_build", "build_script_main"];

pub(crate) const TEST_MARKER: &str = "rustc_test_marker";

pub(crate) fn excluded<'tcx>(tcx: TyCtxt<'tcx>) -> bool {
    let local_crate_name = tcx.crate_name(LOCAL_CRATE);

    let excluded_crate = EXCLUDED_CRATES
        .iter()
        .any(|krate| *krate == local_crate_name.as_str());

    let trybuild = std::env::var(ENV_TARGET_DIR)
        .map(|d| d.ends_with("trybuild"))
        .unwrap_or(false);

    excluded_crate || trybuild
}

pub(crate) fn run_analysis_shared<'tcx>(tcx: TyCtxt<'tcx>) {
    let crate_name = format!("{}", tcx.crate_name(LOCAL_CRATE));
    let crate_id = tcx.sess.local_stable_crate_id().to_u64();

    CRATE_NAME.get_or_init(|| crate_name.clone());
    CRATE_ID.get_or_init(|| crate_id);

    //##############################################################################################################
    // 2. Determine which functions represent tests and store the names of those nodes on the filesystem

    let mut tests: Vec<String> = Vec::new();
    for def_id in tcx.mir_keys(()) {
        for attr in tcx.get_attrs_unchecked(def_id.to_def_id()) {
            if attr.name_or_empty().to_ident_string() == TEST_MARKER {
                tests.push(def_id_name(tcx, def_id.to_def_id(), &[]));
            }
        }
    }

    if tests.len() > 0 {
        write_to_file(
            tests.join("\n").to_string(),
            PATH_BUF.get().unwrap().clone(),
            |buf| get_test_path(buf, &crate_name, crate_id),
            false,
        );
    }

    debug!("Exported tests for {}", crate_name);
}

pub fn export_checksums_and_changes(consider_added_vtable_entries: bool) {
    if let Some(crate_name) = CRATE_NAME.get() {
        let crate_id = *CRATE_ID.get().unwrap();

        let mut new_checksums = NEW_CHECKSUMS.get().unwrap().lock().unwrap();
        let new_checksums_vtbl = NEW_CHECKSUMS_VTBL.get().unwrap().lock().unwrap();

        //##############################################################################################################
        // 3. Import checksums

        let old_checksums = {
            let checksums_path_buf =
                get_checksums_path(PATH_BUF.get().unwrap().clone(), &crate_name, crate_id);

            let maybe_checksums = read(checksums_path_buf);

            if let Ok(checksums) = maybe_checksums {
                Checksums::from(checksums.as_slice())
            } else {
                Checksums::new()
            }
        };

        let old_checksums_vtbl = {
            let checksums_path_buf =
                get_checksums_vtbl_path(PATH_BUF.get().unwrap().clone(), &crate_name, crate_id);

            let maybe_checksums = read(checksums_path_buf);

            if let Ok(checksums) = maybe_checksums {
                Checksums::from(checksums.as_slice())
            } else {
                Checksums::new()
            }
        };

        debug!("Imported checksums for {}", crate_name);

        //##############################################################################################################
        // 4. Calculate new checksums and names of changed nodes and write this information to filesystem

        let mut changed_nodes = HashSet::new();

        let names = NODES.get().unwrap().lock().unwrap();

        trace!("Checksums: {:?}", new_checksums);

        for name in names.iter() {
            trace!("Checking {}", name);
            let changed = {
                let maybe_new = new_checksums.get(name);
                let maybe_old = old_checksums.get(name);

                match (maybe_new, maybe_old) {
                    (None, None) => panic!("Did not find checksum for {}. This may happen when RustyRTS is interrupted and later invoked again. Just do `cargo clean` and invoke it again.", name),
                    (None, Some(checksums)) => {
                        new_checksums.insert(name.clone(), checksums.clone());
                        false
                    }
                    (Some(_), None) => true,
                    (Some(new), Some(old)) => new != old,
                }
            };
            if changed {
                changed_nodes.insert(name.clone());
            }
        }

        for name in new_checksums_vtbl.keys() {
            let changed = {
                let maybe_new = new_checksums_vtbl.get(name);
                let maybe_old = old_checksums_vtbl.get(name);

                // Only in dynamic RustyRTS:
                // We only need to consider functions that are no longer pointed to by the vtable entries
                // (dynamic dispatch may call a different function in the new revision)

                match (maybe_new, maybe_old) {
                    (None, _) => panic!("Did not find checksum for vtable entry {}. This may happen when RustyRTS is interrupted and later invoked again. Just do `cargo clean` and invoke it again.", name),
                    (Some(_), None) => false,
                    (Some(new), Some(old)) => {
                        if consider_added_vtable_entries {
                            old != new
                        } else {
                            old.difference(new).count() != 0
                        }
                     },
                }
            };

            if changed {
                changed_nodes.insert(name.clone());
            }
        }

        // Also consider nodes that do no longer have a vtable entry pointing at them as changed
        // (dynamic dispatch may call a different function in the new revision)

        for node in old_checksums_vtbl.keys() {
            if !new_checksums_vtbl.keys().contains(node) {
                changed_nodes.insert(node.clone());
            }
        }

        write_to_file(
            Into::<Vec<u8>>::into(&*new_checksums),
            PATH_BUF.get().unwrap().clone(),
            |buf| get_checksums_path(buf, &crate_name, crate_id),
            false,
        );

        write_to_file(
            Into::<Vec<u8>>::into(&*new_checksums_vtbl),
            PATH_BUF.get().unwrap().clone(),
            |buf| get_checksums_vtbl_path(buf, &crate_name, crate_id),
            false,
        );

        write_to_file(
            changed_nodes.into_iter().join("\n").to_string(),
            PATH_BUF.get().unwrap().clone(),
            |buf| get_changes_path(buf, &crate_name, crate_id),
            false,
        );

        debug!("Exported changes for {}", crate_name);
    }
}
