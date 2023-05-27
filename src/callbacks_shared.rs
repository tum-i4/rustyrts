use itertools::Itertools;
use log::{debug, trace};
use once_cell::sync::OnceCell;
use rustc_data_structures::sync::Ordering::SeqCst;
use rustc_hir::def_id::LOCAL_CRATE;
use rustc_middle::ty::{InstanceDef, PolyTraitRef, TyCtxt, VtblEntry};
use std::{
    collections::HashSet,
    fs::read,
    mem::transmute,
    sync::{atomic::AtomicUsize, Mutex},
};

use crate::{
    checksums::{get_checksum_vtbl_entry, insert_hashmap, Checksums},
    fs_utils::{
        get_changes_path, get_checksums_path, get_checksums_vtbl_path, get_test_path, write_to_file,
    },
    names::def_id_name,
    static_rts::callback::PATH_BUF,
};

#[cfg(feature = "ctfe")]
use crate::fs_utils::get_checksums_ctfe_path;

pub(crate) static OLD_VTABLE_ENTRIES: AtomicUsize = AtomicUsize::new(0);

pub(crate) static CRATE_NAME: OnceCell<String> = OnceCell::new();
pub(crate) static CRATE_ID: OnceCell<u64> = OnceCell::new();

pub(crate) static NODES: OnceCell<Mutex<HashSet<String>>> = OnceCell::new();
pub(crate) static NEW_CHECKSUMS: OnceCell<Mutex<Checksums>> = OnceCell::new();
pub(crate) static NEW_CHECKSUMS_VTBL: OnceCell<Mutex<Checksums>> = OnceCell::new();

#[cfg(feature = "ctfe")]
pub(crate) static NODES_CTFE: OnceCell<Mutex<HashSet<String>>> = OnceCell::new();
#[cfg(feature = "ctfe")]
pub(crate) static NEW_CHECKSUMS_CTFE: OnceCell<Mutex<Checksums>> = OnceCell::new();

const EXCLUDED_CRATES: &[&str] = &["build_script_build", "build_script_main"];

pub(crate) const TEST_MARKER: &str = "rustc_test_marker";

pub(crate) fn excluded<'tcx>(tcx: TyCtxt<'tcx>) -> bool {
    let local_crate_name = tcx.crate_name(LOCAL_CRATE);
    EXCLUDED_CRATES
        .iter()
        .any(|krate| *krate == local_crate_name.as_str())
}

pub(crate) fn custom_vtable_entries<'tcx>(
    tcx: TyCtxt<'tcx>,
    key: PolyTraitRef<'tcx>,
) -> &'tcx [VtblEntry<'tcx>] {
    let content = OLD_VTABLE_ENTRIES.load(SeqCst);

    // SAFETY: At this address, the original vtable_entries() function has been stored before.
    // We reinterpret it as a function.
    let orig_function = unsafe {
        transmute::<usize, fn(_: TyCtxt<'tcx>, _: PolyTraitRef<'tcx>) -> &'tcx [VtblEntry<'tcx>]>(
            content,
        )
    };

    let result = orig_function(tcx, key);

    for entry in result {
        if let VtblEntry::Method(instance) = entry {
            let def_id = match instance.def {
                InstanceDef::Item(item) => item.did,
                InstanceDef::Intrinsic(def_id) => def_id,
                InstanceDef::VTableShim(def_id) => def_id,
                InstanceDef::ReifyShim(def_id) => def_id,
                InstanceDef::FnPtrShim(def_id, _) => def_id,
                InstanceDef::Virtual(def_id, _) => def_id,
                InstanceDef::ClosureOnceShim {
                    call_once,
                    track_caller: _,
                } => call_once,
                InstanceDef::DropGlue(def_id, _) => def_id,
                InstanceDef::CloneShim(def_id, _) => def_id,
            };

            let name = def_id_name(tcx, def_id);
            let checksum = get_checksum_vtbl_entry(tcx, &entry);
            debug!("Considering {:?} in checksums of {}", instance, name);

            insert_hashmap(
                &mut *NEW_CHECKSUMS_VTBL.get().unwrap().lock().unwrap(),
                name,
                checksum,
            )
        }
    }

    result
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
                tests.push(def_id_name(tcx, def_id.to_def_id()));
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

pub fn export_checksums_and_changes() {
    if let Some(crate_name) = CRATE_NAME.get() {
        let crate_id = *CRATE_ID.get().unwrap();

        let mut new_checksums = NEW_CHECKSUMS.get().unwrap().lock().unwrap();
        let new_checksums_vtbl = NEW_CHECKSUMS_VTBL.get().unwrap().lock().unwrap();

        #[cfg(feature = "ctfe")]
        let mut new_checksums_ctfe = NEW_CHECKSUMS_CTFE.get().unwrap().lock().unwrap();

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

        #[cfg(feature = "ctfe")]
        let old_checksums_ctfe = {
            let checksums_path_buf =
                get_checksums_ctfe_path(PATH_BUF.get().unwrap().clone(), &crate_name, crate_id);

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
                    (None, None) => unreachable!(),
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

        #[cfg(feature = "ctfe")]
        let names_ctfe = NODES_CTFE.get().unwrap().lock().unwrap();

        #[cfg(feature = "ctfe")]
        for name in names_ctfe.iter() {
            let changed = {
                let maybe_new = new_checksums_ctfe.get(name);
                let maybe_old = old_checksums_ctfe.get(name);

                match (maybe_new, maybe_old) {
                    (None, None) => unreachable!(),
                    (None, Some(checksums)) => {
                        new_checksums_ctfe.insert(name.clone(), checksums.clone());
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

                match (maybe_new, maybe_old) {
                    (None, None) => unreachable!(),
                    (None, Some(_)) => unreachable!(),
                    (Some(_), None) => true,
                    (Some(new), Some(old)) => new != old,
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

        #[cfg(feature = "ctfe")]
        write_to_file(
            Into::<Vec<u8>>::into(&*new_checksums_ctfe),
            PATH_BUF.get().unwrap().clone(),
            |buf| get_checksums_ctfe_path(buf, &crate_name, crate_id),
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
