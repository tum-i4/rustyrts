use itertools::Itertools;
use once_cell::sync::OnceCell;

use rustc_data_structures::sync::Ordering::SeqCst;
use rustc_hir::def_id::LOCAL_CRATE;
use rustc_middle::ty::{Instance, TyCtxt};
use rustc_middle::{
    mir::mono::MonoItem,
    ty::{PolyTraitRef, VtblEntry},
};
use std::{
    collections::HashSet,
    fs::read,
    string::ToString,
    sync::{atomic::AtomicUsize, Mutex},
};
use std::{env, fs::remove_file};
use std::{mem::transmute, path::Path};
use tracing::{debug, trace};

use crate::{
    checksums::{get_checksum_body, insert_hashmap},
    const_visitor::ResolvingConstVisitor,
    fs_utils::append_to_file,
};
use crate::{
    checksums::{get_checksum_vtbl_entry, Checksums},
    constants::{ENV_DOCTESTED, ENV_TARGET_DIR},
    fs_utils::{write_to_file, CacheFileKind, CacheKind, ChecksumKind},
    names::def_id_name,
};
use crate::{
    constants::{ENV_COMPILE_MODE, ENV_SKIP_ANALYSIS},
    fs_utils::CacheFileDescr,
};

pub static OLD_VTABLE_ENTRIES: AtomicUsize = AtomicUsize::new(0);
pub static NEW_CHECKSUMS_VTBL: OnceCell<Mutex<Checksums>> = OnceCell::new();

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

pub struct RTSContext {
    pub crate_name: String,
    pub compile_mode: CompileMode,
    pub doctest_name: Option<String>,

    pub new_checksums: OnceCell<Checksums>,
    pub new_checksums_vtbl: OnceCell<Checksums>,
    pub new_checksums_const: OnceCell<Checksums>,
    pub old_checksums: OnceCell<Checksums>,
    pub old_checksums_vtbl: OnceCell<Checksums>,
    pub old_checksums_const: OnceCell<Checksums>,
}

impl RTSContext {
    pub fn new(
        crate_name: String,
        compile_mode: CompileMode,
        doctest_name: Option<String>,
    ) -> Self {
        Self {
            crate_name,
            compile_mode,
            doctest_name,
            new_checksums: OnceCell::new(),
            new_checksums_vtbl: OnceCell::new(),
            new_checksums_const: OnceCell::new(),
            old_checksums: OnceCell::new(),
            old_checksums_vtbl: OnceCell::new(),
            old_checksums_const: OnceCell::new(),
        }
    }
}

#[derive(PartialEq, Debug)]
pub enum CompileMode {
    Build,
    Test,
    Doctest,
}

impl TryFrom<&str> for CompileMode {
    type Error = ();

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "Build" => Ok(Self::Build),
            "Test" => Ok(Self::Test),
            "Doctest" => Ok(Self::Doctest),
            _ => Err(()),
        }
    }
}

impl AsRef<str> for CompileMode {
    fn as_ref(&self) -> &str {
        match self {
            CompileMode::Build => "Build",
            CompileMode::Test => "Test",
            CompileMode::Doctest => "Doctest",
        }
    }
}

pub trait AnalysisCallback<'tcx>: ChecksumsCallback {
    fn init_analysis(&mut self, tcx: TyCtxt<'tcx>) -> RTSContext {
        let compile_mode = std::env::var(ENV_COMPILE_MODE)
            .map(|s| CompileMode::try_from(s.as_str()).expect("Failed to convert compile mode"))
            .expect("Failed to find compile mode");

        let (crate_name, doctest_name) = if compile_mode == CompileMode::Doctest {
            let doctest_name = {
                tcx.mir_keys(())
                    .into_iter()
                    .filter_map(|def_id| {
                        let name = def_id_name(tcx, def_id.to_def_id(), false, true);
                        trace!("Searching for doctest function - Checking {:?}", name);
                        name.strip_prefix(DOCTEST_PREFIX)
                            .filter(|suffix| !suffix.contains("::"))
                            .map(ToString::to_string)
                    })
                    .unique()
                    .exactly_one()
                    .expect("Did not find exactly one suitable doctest name")
            };
            let crate_name = std::env::var(ENV_DOCTESTED).unwrap();

            (crate_name, Some(doctest_name))
        } else {
            let crate_name = format!("{}", tcx.crate_name(LOCAL_CRATE));

            (crate_name, None)
        };

        let context = RTSContext::new(crate_name, compile_mode, doctest_name);
        context.new_checksums.get_or_init(Checksums::new);
        context.new_checksums_const.get_or_init(Checksums::new);

        NEW_CHECKSUMS_VTBL.get_or_init(|| Mutex::new(Checksums::new()));
        context
    }

    fn run_analysis_shared(&mut self, tcx: TyCtxt<'tcx>) {
        if self.context().compile_mode == CompileMode::Test {
            self.run_analysis_tests(tcx);
        }

        let RTSContext {
            new_checksums,
            new_checksums_const,
            ..
        } = self.context_mut();

        //##############################################################################################################
        // Collect all MIR bodies that are relevant for code generation

        let code_gen_units = tcx.collect_and_partition_mono_items(()).1;

        let bodies = code_gen_units
            .iter()
            .flat_map(|c| c.items().keys())
            .filter_map(|m| match m {
                MonoItem::Fn(instance) => Some(instance),
                _ => None,
            })
            .map(Instance::def_id)
            //.filter(|d| d.is_local()) // It is not feasible to only analyze local MIR
            .filter(|d| tcx.is_mir_available(d))
            .unique()
            .map(|d| tcx.optimized_mir(d))
            .collect_vec();

        //##########################################################################################################
        // Calculate checksum of every MIR body and the consts that it uses

        for body in &bodies {
            let name = def_id_name(tcx, body.source.def_id(), false, true);

            let checksums_const = ResolvingConstVisitor::find_consts(tcx, body);
            for checksum in checksums_const {
                insert_hashmap(
                    &mut *new_checksums_const.get_mut().unwrap(),
                    &name,
                    checksum,
                );
            }

            let checksum = get_checksum_body(tcx, body);
            insert_hashmap(&mut *new_checksums.get_mut().unwrap(), &name, checksum);
        }
    }

    fn run_analysis_tests(&self, tcx: TyCtxt<'tcx>) {
        let path = self.path();
        let RTSContext {
            crate_name,
            compile_mode,
            ..
        } = self.context();

        //##############################################################################################################
        // Determine which functions represent tests and store the names of those nodes on the filesystem

        let mut tests: Vec<String> = Vec::new();

        for def_id in tcx.mir_keys(()) {
            for attr in tcx.get_attrs_unchecked(def_id.to_def_id()) {
                if attr.name_or_empty().to_ident_string() == TEST_MARKER {
                    tests.push(def_id_name(tcx, def_id.to_def_id(), false, false));
                }
            }
        }

        write_to_file(
            tests.join("\n").to_string() + "\n",
            CacheKind::General.map(path.to_path_buf()),
            |buf| {
                CacheFileDescr::new(
                    crate_name,
                    Some(compile_mode.as_ref()),
                    None,
                    CacheFileKind::Tests,
                )
                .apply(buf);
            },
            false,
        );

        debug!("Exported tests for {}", crate_name);
    }

    fn custom_vtable_entries(
        tcx: TyCtxt<'tcx>,
        key: PolyTraitRef<'tcx>,
        suffix: &str,
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

        if !excluded(tcx.crate_name(LOCAL_CRATE).as_str()) {
            for entry in result {
                if let VtblEntry::Method(instance) = entry {
                    let def_id = instance.def_id();
                    if !tcx.is_closure(def_id) && !tcx.is_fn_trait(key.def_id()) {
                        let checksum = get_checksum_vtbl_entry(tcx, entry);
                        let name = def_id_name(tcx, def_id, false, true) + suffix;

                        trace!("Considering {:?} in checksums of {}", instance, name);

                        insert_hashmap(
                            &mut *NEW_CHECKSUMS_VTBL.get().unwrap().lock().unwrap(),
                            &name,
                            checksum,
                        );
                    }
                }
            }
        }

        result
    }
}

pub trait ChecksumsCallback {
    fn path(&self) -> &Path;
    fn context(&self) -> &RTSContext;
    fn context_mut(&mut self) -> &mut RTSContext;

    fn import_checksums(&self, kind: ChecksumKind, remove: bool) -> Checksums {
        let path = CacheKind::General.map(self.path().to_path_buf());
        let RTSContext {
            crate_name,
            compile_mode,
            doctest_name,
            ..
        } = self.context();

        //#################################################################################################################
        // Import old checksums

        debug!(
            "Importing {:?} for {},{:?},{:?}",
            kind, crate_name, compile_mode, doctest_name
        );

        let old_checksums = {
            let mut checksums_path_buf = path.clone();
            CacheFileDescr::new(
                crate_name,
                Some(compile_mode.as_ref()),
                doctest_name.as_deref(),
                CacheFileKind::Checksums(kind),
            )
            .apply(&mut checksums_path_buf);

            let maybe_checksums = read(checksums_path_buf.as_path());

            if let Ok(checksums) = maybe_checksums {
                if remove {
                    remove_file(checksums_path_buf.as_path()).unwrap();
                }
                Checksums::from(checksums.as_slice())
            } else {
                Checksums::new()
            }
        };

        old_checksums
    }

    fn export_changes(&self, cache_kind: CacheKind) {
        let RTSContext {
            crate_name,
            compile_mode,
            doctest_name,
            new_checksums,
            new_checksums_vtbl,
            new_checksums_const,
            old_checksums,
            old_checksums_vtbl,
            old_checksums_const,
        } = self.context();

        debug!("Exporting changes for {}", crate_name);

        let from_new_revision = match cache_kind {
            CacheKind::Static => true,
            CacheKind::Dynamic => false, // IMPORTANT: dynamic RTS selects based on the old revision
            CacheKind::General => panic!("Got invalid cache kind for changes"),
        };

        let changed_nodes = calculate_changes(
            from_new_revision,
            new_checksums.get().unwrap(),
            new_checksums_vtbl.get().unwrap(),
            new_checksums_const.get().unwrap(),
            old_checksums.get().unwrap(),
            old_checksums_vtbl.get().unwrap(),
            old_checksums_const.get().unwrap(),
        );

        if !changed_nodes.is_empty() {
            write_to_file(
                changed_nodes.into_iter().join("\n"),
                cache_kind.map(self.path().to_path_buf()),
                |buf| {
                    CacheFileDescr::new(
                        crate_name,
                        Some(compile_mode.as_ref()),
                        doctest_name.as_deref(),
                        CacheFileKind::Changes,
                    )
                    .apply(buf);
                },
                true, // IMPORTANT: append changes to handle changing files in between compiling
            );
        }
    }

    fn export_checksums(&self, kind: ChecksumKind, checksums: &Checksums, append: bool) {
        let path = CacheKind::General.map(self.path().to_path_buf());
        let RTSContext {
            crate_name,
            compile_mode,
            doctest_name,
            ..
        } = self.context();

        debug!("Exporting {:?} checksums for {}", kind, crate_name);

        let descr = CacheFileDescr::new(
            crate_name,
            Some(compile_mode.as_ref()),
            doctest_name.as_deref(),
            CacheFileKind::Checksums(kind),
        );

        if append {
            append_to_file(Into::<Vec<u8>>::into(checksums), path.clone(), |path_buf| {
                descr.apply(path_buf);
            });
        } else {
            write_to_file(
                Into::<Vec<u8>>::into(checksums),
                path.clone(),
                |path_buf| descr.apply(path_buf),
                false,
            );
        }
    }
}

fn calculate_changes(
    from_new_revision: bool,
    new_checksums: &Checksums,
    new_checksums_vtbl: &Checksums,
    new_checksums_const: &Checksums,
    old_checksums: &Checksums,
    old_checksums_vtbl: &Checksums,
    old_checksums_const: &Checksums,
) -> HashSet<String> {
    //#################################################################################################################
    // Calculate names of changed nodes and write this information to filesystem

    let mut changed_nodes = HashSet::new();

    // We only consider nodes from the new revision
    // (Dynamic: if something in the old revision has been removed, there must be a change to some other function)
    for name in new_checksums.keys() {
        trace!("Checking {}", name);
        let maybe_new = new_checksums.get(name);
        let maybe_old = old_checksums.get(name);

        let changed = {
            match (maybe_new, maybe_old) {
                (None, _) => unreachable!(),
                (Some(_), None) => true,
                (Some(new), Some(old)) => new != old,
            }
        };

        if changed {
            debug!(
                "Changed due to regular checksums: {} {:?}/{:?}",
                name, maybe_old, maybe_new
            );
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
                    (None, _) => panic!("Did not find checksum for vtable entry {name}. This may happen when RustyRTS is interrupted and later invoked again. Just do `cargo clean` and invoke it again."),
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
