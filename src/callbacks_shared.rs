use itertools::Itertools;
use once_cell::sync::OnceCell;

use rustc_hir::def_id::LOCAL_CRATE;
use rustc_middle::mir::mono::MonoItem;
use rustc_middle::ty::TyCtxt;
use std::env;
use std::path::PathBuf;
use std::{
    collections::HashSet,
    fs::read,
    sync::{atomic::AtomicUsize, Mutex},
};
use tracing::{debug, trace};

use crate::constants::ENV_SKIP_ANALYSIS;
use crate::{
    checksums::Checksums,
    constants::{
        ENDING_CHANGES, ENDING_CHECKSUM, ENDING_CHECKSUM_CONST, ENDING_CHECKSUM_VTBL, ENDING_TEST,
        ENV_TARGET_DIR,
    },
    fs_utils::{init_path, link_to_file, write_to_file},
    names::def_id_name,
};
use crate::{
    checksums::{get_checksum_body, insert_hashmap},
    const_visitor::ResolvingConstVisitor,
};

pub(crate) static OLD_VTABLE_ENTRIES: AtomicUsize = AtomicUsize::new(0);

pub(crate) static PATH_BUF: OnceCell<PathBuf> = OnceCell::new();
pub(crate) static PATH_BUF_DOCTESTS: OnceCell<PathBuf> = OnceCell::new();

pub(crate) static CRATE_NAME: OnceCell<String> = OnceCell::new();
pub(crate) static CRATE_ID: OnceCell<Option<u64>> = OnceCell::new();

pub(crate) static NEW_CHECKSUMS: OnceCell<Mutex<Checksums>> = OnceCell::new();
pub(crate) static NEW_CHECKSUMS_VTBL: OnceCell<Mutex<Checksums>> = OnceCell::new();
pub(crate) static NEW_CHECKSUMS_CONST: OnceCell<Mutex<Checksums>> = OnceCell::new();

const EXCLUDED_CRATES: &[&str] = &["build_script_build", "build_script_main"];

pub(crate) const TEST_MARKER: &str = "rustc_test_marker";
pub const DOCTEST_PREFIX: &str = "rust_out::_doctest_main_";

pub(crate) static EXCLUDED: OnceCell<bool> = OnceCell::new();
static NO_INSTRUMENTATION: OnceCell<bool> = OnceCell::new();

pub(crate) fn excluded(crate_name: &str) -> bool {
    *EXCLUDED.get_or_init(|| {
        let exclude = env::var(ENV_SKIP_ANALYSIS).is_ok() || no_instrumentation(crate_name);
        if exclude {
            debug!("Excluding crate {}", crate_name);
        }
        exclude
    })
}

pub(crate) fn no_instrumentation(crate_name: &str) -> bool {
    *NO_INSTRUMENTATION.get_or_init(|| {
        let excluded_crate = EXCLUDED_CRATES.iter().any(|krate| *krate == crate_name);

        let trybuild = std::env::var(ENV_TARGET_DIR)
            .map(|d| d.ends_with("trybuild"))
            .unwrap_or(false);

        let no_instrumentation = excluded_crate || trybuild;
        if no_instrumentation {
            debug!("Not instrumenting crate {}", crate_name);
        }
        no_instrumentation
    })
}

pub(crate) fn init_analysis(tcx: TyCtxt<'_>) {
    CRATE_NAME.get_or_init(|| (format!("{}", tcx.crate_name(LOCAL_CRATE))).clone());
    CRATE_ID.get_or_init(|| Some(tcx.stable_crate_id(LOCAL_CRATE).as_u64()));

    NEW_CHECKSUMS.get_or_init(|| Mutex::new(Checksums::new()));
    NEW_CHECKSUMS_VTBL.get_or_init(|| Mutex::new(Checksums::new()));
    NEW_CHECKSUMS_CONST.get_or_init(|| Mutex::new(Checksums::new()));
}

pub(crate) fn run_analysis_shared(tcx: TyCtxt<'_>) {
    let path = PATH_BUF.get().unwrap();

    let crate_name = CRATE_NAME.get().unwrap();
    let crate_id = *CRATE_ID.get().unwrap();

    let mut new_checksums = NEW_CHECKSUMS.get().unwrap().lock().unwrap();
    let mut new_checksums_const = NEW_CHECKSUMS_CONST.get().unwrap().lock().unwrap();

    //##############################################################################################################
    // Collect all MIR bodies that are relevant for code generation

    let code_gen_units = tcx.collect_and_partition_mono_items(()).1;

    let bodies = code_gen_units
        .iter()
        .flat_map(|c| c.items().keys())
        .filter(|m| matches!(m, MonoItem::Fn(_)))
        .map(|m| {
            let MonoItem::Fn(instance) = m else {
                unreachable!()
            };
            instance
        })
        .map(|i| i.def_id())
        //.filter(|d| d.is_local()) // It is not feasible to only analyze local MIR
        .filter(|d| tcx.is_mir_available(d))
        .unique()
        .map(|d| tcx.optimized_mir(d))
        .collect_vec();

    //##########################################################################################################
    // 2. Calculate checksum of every MIR body and the consts that it uses

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

    if !tests.is_empty() {
        write_to_file(
            tests.join("\n").to_string() + "\n",
            path.clone(),
            |buf| init_path(buf, crate_name, crate_id, ENDING_TEST),
            false,
        );
    }

    debug!("Exported tests for {}", crate_name);
}

pub fn import_checksums(
    path: PathBuf,
    crate_name: &String,
    crate_id: Option<u64>,
    ending: &str,
) -> Checksums {
    //#################################################################################################################
    // Import old checksums

    let old_checksums = {
        let mut checksums_path_buf = path.clone();
        init_path(&mut checksums_path_buf, crate_name, crate_id, ending);

        let maybe_checksums = read(checksums_path_buf);

        if let Ok(checksums) = maybe_checksums {
            Checksums::from(checksums.as_slice())
        } else {
            Checksums::new()
        }
    };

    debug!("Imported {} for {}", ending, crate_name);

    old_checksums
}

pub fn calculate_changes(
    from_new_revision: bool,
    old_checksums: &Checksums,
    old_checksums_vtbl: &Checksums,
    old_checksums_const: &Checksums,
    new_checksums: &Checksums,
    new_checksums_vtbl: &Checksums,
    new_checksums_const: &Checksums,
) -> HashSet<String> {
    //#################################################################################################################
    // Calculate names of changed nodes and write this information to filesystem

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
        (new_checksums_vtbl, old_checksums_vtbl)
    } else {
        (old_checksums_vtbl, new_checksums_vtbl)
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

    changed_nodes
}

pub(crate) fn export_changes(
    from_new_revision: bool,
    path: PathBuf,
    crate_name: &String,
    crate_id: Option<u64>,
    old_checksums: &Checksums,
    old_checksums_vtbl: &Checksums,
    old_checksums_const: &Checksums,
    new_checksums: &Checksums,
    new_checksums_vtbl: &Checksums,
    new_checksums_const: &Checksums,
) {
    let changed_nodes = calculate_changes(
        from_new_revision,
        old_checksums,
        old_checksums_vtbl,
        old_checksums_const,
        new_checksums,
        new_checksums_vtbl,
        new_checksums_const,
    );

    write_to_file(
        changed_nodes.into_iter().join("\n").to_string() + "\n",
        path.clone(),
        |buf| init_path(buf, crate_name, crate_id, ENDING_CHANGES),
        true,
    );

    debug!("Exported changes for {}", crate_name);
}

pub(crate) fn export_checksums(
    path: PathBuf,
    crate_name: &String,
    crate_id: Option<u64>,
    new_checksums: &Checksums,
    new_checksums_vtbl: &Checksums,
    new_checksums_const: &Checksums,
    append: bool,
) {
    write_to_file(
        Into::<Vec<u8>>::into(&*new_checksums),
        path.clone(),
        |buf| init_path(buf, crate_name, crate_id, ENDING_CHECKSUM),
        append,
    );

    write_to_file(
        Into::<Vec<u8>>::into(&*new_checksums_vtbl),
        path.clone(),
        |buf| init_path(buf, crate_name, crate_id, ENDING_CHECKSUM_VTBL),
        append,
    );

    write_to_file(
        Into::<Vec<u8>>::into(&*new_checksums_const),
        path.clone(),
        |buf| init_path(buf, crate_name, crate_id, ENDING_CHECKSUM_CONST),
        append,
    );

    debug!("Exported checksums for {}", crate_name);
}

pub(crate) fn link_checksums(
    path_orig: PathBuf,
    path: PathBuf,
    crate_name: &String,
    crate_id: Option<u64>,
) {
    link_to_file(path_orig.clone(), path.clone(), |buf| {
        init_path(buf, crate_name, crate_id, ENDING_CHECKSUM)
    });

    link_to_file(path_orig.clone(), path.clone(), |buf| {
        init_path(buf, crate_name, crate_id, ENDING_CHECKSUM_VTBL)
    });

    link_to_file(path_orig.clone(), path.clone(), |buf| {
        init_path(buf, crate_name, crate_id, ENDING_CHECKSUM_CONST)
    });

    debug!("Linked checksums for {}", crate_name);
}
