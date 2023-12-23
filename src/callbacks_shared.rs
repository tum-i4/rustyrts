use itertools::Itertools;
use log::{debug, trace};
use once_cell::sync::OnceCell;
use rustc_hir::def_id::LOCAL_CRATE;
use rustc_middle::mir::mono::MonoItem;
use rustc_middle::ty::TyCtxt;
use std::env;
use std::{
    collections::HashSet,
    fs::read,
    sync::{atomic::AtomicUsize, Mutex},
};

use crate::{checksums::{get_checksum_body, insert_hashmap}, const_visitor::ResolvingConstVisitor};
use crate::constants::ENV_SKIP_ANALYSIS;
use crate::{
    checksums::Checksums,
    constants::ENV_TARGET_DIR,
    fs_utils::{
        get_changes_path, get_checksums_const_path, get_checksums_path, get_checksums_vtbl_path,
        get_test_path, write_to_file,
    },
    names::def_id_name,
    static_rts::callback::PATH_BUF,
};

pub(crate) static OLD_VTABLE_ENTRIES: AtomicUsize = AtomicUsize::new(0);

pub(crate) static CRATE_NAME: OnceCell<String> = OnceCell::new();
pub(crate) static CRATE_ID: OnceCell<u64> = OnceCell::new();

pub(crate) static NEW_CHECKSUMS: OnceCell<Mutex<Checksums>> = OnceCell::new();
pub(crate) static NEW_CHECKSUMS_VTBL: OnceCell<Mutex<Checksums>> = OnceCell::new();
pub(crate) static NEW_CHECKSUMS_CONST: OnceCell<Mutex<Checksums>> = OnceCell::new();

const EXCLUDED_CRATES: &[&str] = &["build_script_build", "build_script_main"];

pub(crate) const TEST_MARKER: &str = "rustc_test_marker";

pub(crate) static EXCLUDED: OnceCell<bool> = OnceCell::new();
static NO_INSTRUMENTATION: OnceCell<bool> = OnceCell::new();

pub(crate) fn excluded<F: Copy + Fn() -> String>(getter_crate_name: F) -> bool {
    *EXCLUDED.get_or_init(|| {
        let exclude = env::var(ENV_SKIP_ANALYSIS).is_ok() || no_instrumentation(getter_crate_name);
        if exclude {
            trace!("Excluding crate {}", getter_crate_name());
        }
        exclude
    })
}

pub(crate) fn no_instrumentation<F: Copy + Fn() -> String>(getter_crate_name: F) -> bool {
    *NO_INSTRUMENTATION.get_or_init(|| {
        let excluded_crate = EXCLUDED_CRATES
            .iter()
            .any(|krate| *krate == getter_crate_name());

        let trybuild = std::env::var(ENV_TARGET_DIR)
            .map(|d| d.ends_with("trybuild"))
            .unwrap_or(false);

        let no_instrumentation = excluded_crate || trybuild;
        if no_instrumentation {
            trace!("Not instrumenting crate {}", getter_crate_name());
        }
        no_instrumentation
    })
}

pub(crate) fn run_analysis_shared<'tcx>(tcx: TyCtxt<'tcx>) {
    let crate_name = format!("{}", tcx.crate_name(LOCAL_CRATE));
    let crate_id = tcx.sess.local_stable_crate_id().to_u64();

    //##############################################################################################################
    // Collect all MIR bodies that are relevant for code generation

    let code_gen_units = tcx.collect_and_partition_mono_items(()).1;

    let bodies = code_gen_units
        .iter()
        .flat_map(|c| c.items().keys())
        .filter(|m| if let MonoItem::Fn(_) = m { true } else { false })
        .map(|m| {
            let MonoItem::Fn(instance) = m else {unreachable!()};
            instance
        })
        .map(|i| i.def_id())
        //.filter(|d| d.is_local()) // It is not feasible to only analyze local MIR
        .filter(|d| tcx.is_mir_available(d))
        .unique()
        .map(|d| tcx.optimized_mir(d))
        .collect_vec();

    //##############################################################################################################
    // Continue at shared analysis

    CRATE_NAME.get_or_init(|| crate_name.clone());
    CRATE_ID.get_or_init(|| crate_id);

    //##########################################################################################################
    // 2. Calculate checksum of every MIR body and the consts that it uses

    let mut new_checksums = NEW_CHECKSUMS.get().unwrap().lock().unwrap();
    let mut new_checksums_const = NEW_CHECKSUMS_CONST.get().unwrap().lock().unwrap();

    for body in &bodies {
        let name = def_id_name(tcx, body.source.def_id(), false, true);

        let checksums_const = ResolvingConstVisitor::find_consts(tcx, body);
        for checksum in checksums_const {
            insert_hashmap(&mut *new_checksums_const, &name, checksum);
        }

        let checksum = get_checksum_body(tcx, body);
        insert_hashmap(&mut *new_checksums, &name, checksum);
    }

    //##############################################################################################################
    // 3. Determine which functions represent tests and store the names of those nodes on the filesystem

    let mut tests: Vec<String> = Vec::new();
    for def_id in tcx.mir_keys(()) {
        for attr in tcx.get_attrs_unchecked(def_id.to_def_id()) {
            if attr.name_or_empty().to_ident_string() == TEST_MARKER {
                tests.push(def_id_name(tcx, def_id.to_def_id(), false, false));
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

    trace!("Exported tests for {}", crate_name);
}

pub fn export_checksums_and_changes(from_new_revision: bool) {
    if let Some(crate_name) = CRATE_NAME.get() {
        let crate_id = *CRATE_ID.get().unwrap();

        let new_checksums = NEW_CHECKSUMS.get().unwrap().lock().unwrap();
        let new_checksums_vtbl = NEW_CHECKSUMS_VTBL.get().unwrap().lock().unwrap();
        let new_checksums_const = NEW_CHECKSUMS_CONST.get().unwrap().lock().unwrap();

        //##############################################################################################################
        // Import checksums

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

        let old_checksums_const = {
            let checksums_path_buf =
                get_checksums_const_path(PATH_BUF.get().unwrap().clone(), &crate_name, crate_id);

            let maybe_checksums = read(checksums_path_buf);

            if let Ok(checksums) = maybe_checksums {
                Checksums::from(checksums.as_slice())
            } else {
                Checksums::new()
            }
        };

        trace!("Imported checksums for {}", crate_name);

        //##############################################################################################################
        // 4. Calculate names of changed nodes and write this information to filesystem

        let mut changed_nodes = HashSet::new();

        // We only consider nodes from the new revision
        // (Dynamic: if something in the old revision has been removed, there must be a change to some other function)
        for name in new_checksums.keys() {
            trace!("Checking {}", name);
            let changed = {
                let maybe_new = new_checksums.get(name);
                let maybe_old = old_checksums.get(name);

                match (maybe_new, maybe_old) {
                    (None, _) => unreachable!(),
                    (Some(_), None) => true,
                    (Some(new), Some(old)) => new != old,
                }
            };

            if changed {
                debug!("Changed due to regular checksums: {}", name);
                changed_nodes.insert(name.clone());
            }
        }

        // To properly handle dynamic dispatch, we need to differentiate
        // We consider nodes from the "primary" revision
        // In case of dynamic, this is the old revision (because traces are from the old revision)
        // In case of static, this is the new revision (because graph is build over new revision)
        let (primary_vtbl_checksums, secondary_vtbl_checksums) = if from_new_revision {
            (&*new_checksums_vtbl, &old_checksums_vtbl)
        } else {
            (&old_checksums_vtbl, &*new_checksums_vtbl)
        };

        // We consider nodes from the "primary" revision
        for name in primary_vtbl_checksums.keys() {
            let changed = {
                let maybe_primary = primary_vtbl_checksums.get(name);
                let maybe_secondary = secondary_vtbl_checksums.get(name);

                match (maybe_primary, maybe_secondary) {
                    (None, _) => panic!("Did not find checksum for vtable entry {}. This may happen when RustyRTS is interrupted and later invoked again. Just do `cargo clean` and invoke it again.", name),
                    (Some(_), None) => {
                        // We consider functions that are not in the secondary set
                        // In case of dynamic: functions that do no longer have an entry pointing to them
                        // In case of static: functions that now have an entry pointing to them
                        true
                    },
                    (Some(primary), Some(secondary)) => {
                        // Respectively if there is an entry that is missing in the secondary set
                        primary.difference(secondary).count() != 0
                     },
                }
            };

            if changed {
                // Set to info, to recognize discrepancies between dynamic and static later on
                debug!("Changed due to vtable checksums: {}", name);
                changed_nodes.insert(name.clone());
            }
        }

        // We only consider nodes from the new revision
        for name in new_checksums_const.keys() {
            let changed = {
                let maybe_new = new_checksums_const.get(name);
                let maybe_old = old_checksums_const.get(name);

                match (maybe_new, maybe_old) {
                    (None, _) => unreachable!(),
                    (Some(_), None) => true,
                    (Some(new), Some(old)) => new != old,
                }
            };

            if changed {
                debug!("Changed due to const checksums: {}", name);
                changed_nodes.insert(name.clone());
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
            Into::<Vec<u8>>::into(&*new_checksums_const),
            PATH_BUF.get().unwrap().clone(),
            |buf| get_checksums_const_path(buf, &crate_name, crate_id),
            false,
        );

        write_to_file(
            changed_nodes.into_iter().join("\n").to_string(),
            PATH_BUF.get().unwrap().clone(),
            |buf| get_changes_path(buf, &crate_name, crate_id),
            false,
        );

        trace!("Exported changes for {}", crate_name);
    }
}
