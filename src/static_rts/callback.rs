use std::{mem::transmute, path::PathBuf};

use crate::{
    callbacks_shared::{
        excluded, export_changes, export_checksums, import_checksums, init_analysis,
        link_checksums, run_analysis_shared, CRATE_ID, CRATE_NAME, DOCTEST_PREFIX, EXCLUDED,
        NEW_CHECKSUMS, NEW_CHECKSUMS_CONST, NEW_CHECKSUMS_VTBL, OLD_VTABLE_ENTRIES, PATH_BUF,
        PATH_BUF_DOCTESTS,
    },
    constants::{ENDING_CHECKSUM, ENDING_CHECKSUM_CONST, ENDING_CHECKSUM_VTBL, ENDING_GRAPH},
    doctest_rts,
    fs_utils::init_path,
    names::{def_path_str_with_substs_with_no_visible_path, mono_def_id_name},
    static_rts::{graph::EdgeType, visitor::collect_test_functions},
};
use crate::{
    constants::SUFFIX_DYN,
    static_rts::visitor::{create_dependency_graph, MonoItemCollectionMode},
};
use itertools::Itertools;
use rustc_data_structures::sync::Ordering;
use rustc_driver::{Callbacks, Compilation};
use rustc_hir::def_id::LOCAL_CRATE;

use rustc_interface::{interface, Queries};
use rustc_middle::ty::{List, PolyTraitRef, TyCtxt, VtblEntry};
use rustc_session::config::CrateType;
use std::sync::atomic::Ordering::SeqCst;
use tracing::{debug, info, trace};

use crate::checksums::{get_checksum_vtbl_entry, insert_hashmap};
use crate::fs_utils::write_to_file;
use crate::names::{def_id_name, IS_COMPILING_DOCTESTS};

pub struct StaticRTSCallbacks {
    compiling_doctests: bool,
}

impl StaticRTSCallbacks {
    pub fn new(
        maybe_path: Option<PathBuf>,
        maybe_doctest_path: Option<PathBuf>,
        compiling_doctests: bool,
    ) -> Self {
        if let Some(path) = maybe_path {
            PATH_BUF.get_or_init(|| path);
        }
        if let Some(path) = maybe_doctest_path {
            PATH_BUF_DOCTESTS.get_or_init(|| path);
        }
        IS_COMPILING_DOCTESTS.store(compiling_doctests, Ordering::SeqCst);
        Self { compiling_doctests }
    }
}

impl Drop for StaticRTSCallbacks {
    fn drop(&mut self) {
        if let Some(path) = PATH_BUF.get() {
            let Some(crate_name) = CRATE_NAME.get() else {
                return;
            };
            let Some(crate_id) = CRATE_ID.get() else {
                return;
            };
            let crate_id = *crate_id;

            if !excluded(crate_name) {
                let new_checksums = NEW_CHECKSUMS.get().unwrap().lock().unwrap();
                let new_checksums_vtbl = NEW_CHECKSUMS_VTBL.get().unwrap().lock().unwrap();
                let new_checksums_const = NEW_CHECKSUMS_CONST.get().unwrap().lock().unwrap();

                if !self.compiling_doctests {
                    let old_checksums =
                        import_checksums(path.clone(), crate_name, crate_id, ENDING_CHECKSUM);
                    let old_checksums_vtbl =
                        import_checksums(path.clone(), crate_name, crate_id, ENDING_CHECKSUM_VTBL);
                    let old_checksums_const =
                        import_checksums(path.clone(), crate_name, crate_id, ENDING_CHECKSUM_CONST);

                    export_changes(
                        true, // IMPORTANT: static RTS selects based on the new revision
                        path.clone(),
                        crate_name,
                        crate_id,
                        &old_checksums,
                        &old_checksums_vtbl,
                        &old_checksums_const,
                        &new_checksums,
                        &new_checksums_vtbl,
                        &new_checksums_const,
                    );
                    if let Some(path_doctests) = PATH_BUF_DOCTESTS.get() {
                        export_changes(
                            true,
                            path_doctests.clone(),
                            crate_name,
                            crate_id,
                            &old_checksums,
                            &old_checksums_vtbl,
                            &old_checksums_const,
                            &new_checksums,
                            &new_checksums_vtbl,
                            &new_checksums_const,
                        );
                    }
                }

                export_checksums(
                    path.clone(),
                    crate_name,
                    crate_id,
                    &new_checksums,
                    &new_checksums_vtbl,
                    &new_checksums_const,
                    self.compiling_doctests,
                );

                if let Some(path_doctests) = PATH_BUF_DOCTESTS.get() {
                    link_checksums(path.clone(), path_doctests.clone(), crate_name, crate_id);
                }
            }
        }
    }
}

impl Callbacks for StaticRTSCallbacks {
    fn config(&mut self, config: &mut interface::Config) {
        // There is no point in analyzing a proc macro that is executed a compile time
        if config
            .opts
            .crate_types
            .iter()
            .any(|t| *t == CrateType::ProcMacro)
        {
            trace!(
                "Excluding crate {}",
                config.opts.crate_name.as_ref().unwrap()
            );
            EXCLUDED.get_or_init(|| true);
        }

        config.opts.unstable_opts.always_encode_mir = true;

        // The only possibility to intercept vtable entries, which I found, is in their local crate
        config.override_queries = Some(|_session, providers| {
            // SAFETY: We store the address of the original vtable_entries function as a usize.
            OLD_VTABLE_ENTRIES.store(providers.vtable_entries as usize, SeqCst);

            providers.vtable_entries = custom_vtable_entries_monomorphized;
        });
    }

    fn after_crate_root_parsing<'tcx>(
        &mut self,
        _compiler: &interface::Compiler,
        queries: &'tcx Queries<'tcx>,
    ) -> Compilation {
        queries.global_ctxt().unwrap().enter(|tcx| {
            if self.compiling_doctests {
                let crate_name = {
                    tcx.mir_keys(())
                        .into_iter()
                        .filter_map(|def_id| {
                            let name = def_id_name(tcx, def_id.to_def_id(), false, true);
                            trace!("Searching for doctest function - Checking {:?}", name);
                            name.strip_prefix(DOCTEST_PREFIX)
                                .filter(|suffix| !suffix.contains("::"))
                                .map(|s| s.to_string())
                        })
                        .dedup()
                        .exactly_one()
                        .expect("Did not find exactly one suitable doctest name")
                };
                let crate_id = None;

                CRATE_NAME.get_or_init(|| crate_name);
                CRATE_ID.get_or_init(|| crate_id);
            }

            init_analysis(tcx);
        });

        Compilation::Continue
    }

    fn after_analysis<'tcx>(
        &mut self,
        _compiler: &interface::Compiler,
        queries: &'tcx Queries<'tcx>,
    ) -> Compilation {
        queries.global_ctxt().unwrap().enter(|tcx| {
            if !excluded(tcx.crate_name(LOCAL_CRATE).as_str()) {
                self.run_analysis(tcx);
            }
        });

        if !self.compiling_doctests {
            Compilation::Continue
        } else {
            Compilation::Stop
        }
    }
}

impl StaticRTSCallbacks {
    fn run_analysis(&mut self, tcx: TyCtxt) {
        if let Some(path) = PATH_BUF.get() {
            let crate_name = CRATE_NAME.get().unwrap();
            let crate_id = *CRATE_ID.get().unwrap();

            let arena = internment::Arena::new();
            let mut graph = create_dependency_graph(tcx, &arena, MonoItemCollectionMode::Lazy);

            if !self.compiling_doctests {
                let tests = tcx.sess.time("dependency_graph_root_collection", || {
                    collect_test_functions(tcx)
                });

                for test in tests {
                    let def_id = test.def_id();
                    let name_trimmed = def_id_name(tcx, def_id, false, true);
                    let name = mono_def_id_name(tcx, def_id, List::empty(), false, false);
                    graph.add_edge(name, name_trimmed, EdgeType::Trimmed);
                }
            }

            debug!("Created graph for {}", crate_name);

            write_to_file(
                graph.to_string(),
                path.clone(),
                |buf| init_path(buf, crate_name, crate_id, ENDING_GRAPH),
                self.compiling_doctests,
            );

            run_analysis_shared(tcx);
        }
        doctest_rts::r#static::doctests_analysis(tcx);
    }
}

fn custom_vtable_entries_monomorphized<'tcx>(
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

    if !excluded(tcx.crate_name(LOCAL_CRATE).as_str()) {
        for entry in result {
            if let VtblEntry::Method(instance) = entry {
                let def_id = instance.def_id();
                if !tcx.is_closure(def_id) && !tcx.is_fn_trait(key.def_id()) {
                    let checksum = get_checksum_vtbl_entry(tcx, entry);
                    let name = def_id_name(tcx, def_id, false, true).to_owned() + SUFFIX_DYN;

                    trace!("Considering {:?} in checksums of {}", instance, name);

                    insert_hashmap(
                        &mut *NEW_CHECKSUMS_VTBL.get().unwrap().lock().unwrap(),
                        &name,
                        checksum,
                    )
                }
            }
        }
    }

    result
}
