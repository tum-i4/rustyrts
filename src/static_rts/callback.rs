use super::graph::{serialize::ArenaSerializable, DependencyGraph};
use crate::fs_utils::write_to_file;
use crate::names::def_id_name;
use crate::{
    callbacks_shared::{
        AnalysisCallback, ChecksumsCallback, RTSContext, NEW_CHECKSUMS_VTBL, OLD_VTABLE_ENTRIES,
    },
    constants::ENV_SKIP_ANALYSIS,
    fs_utils::{CacheFileDescr, CacheFileKind, CacheKind, ChecksumKind},
    names::mono_def_id_name,
    static_rts::{graph::EdgeType, visitor::collect_test_functions},
};
use crate::{
    constants::SUFFIX_DYN,
    static_rts::visitor::{create_dependency_graph, MonoItemCollectionMode},
};
use internment::Arena;
use once_cell::sync::OnceCell;
use rustc_driver::{Callbacks, Compilation};
use rustc_interface::{interface, Queries};
use rustc_middle::ty::{List, TyCtxt};
use std::path::PathBuf;
use std::sync::atomic::Ordering::SeqCst;
use tracing::{debug, trace};

pub struct StaticRTSCallbacks {
    path: PathBuf,
    context: OnceCell<RTSContext>,
}

impl StaticRTSCallbacks {
    pub fn new(target_dir: PathBuf) -> Self {
        Self {
            path: target_dir,
            context: OnceCell::new(),
        }
    }
}

impl<'tcx> AnalysisCallback<'tcx> for StaticRTSCallbacks {}

impl ChecksumsCallback for StaticRTSCallbacks {
    fn path(&self) -> &std::path::Path {
        &self.path
    }

    fn context(&self) -> &RTSContext {
        self.context.get().expect("Context not yet initilaized")
    }

    fn context_mut(&mut self) -> &mut RTSContext {
        self.context.get_mut().expect("Context not yet initilaized")
    }
}

impl<'tcx> GraphAnalysisCallback<'tcx> for StaticRTSCallbacks {
    fn export_graph(&self, tcx: TyCtxt<'tcx>) {
        let RTSContext {
            crate_name,
            compile_mode,
            target,
            doctest_name,
            ..
        } = self.context();

        let arena = internment::Arena::new();

        let graph = {
            let _prof_timer = tcx
                .prof
                .generic_activity("RUSTYRTS_dependency_graph_creation");

            let mut graph = self.create_graph(&arena, tcx);

            let tests = collect_test_functions(tcx);

            for test in tests {
                let def_id = test.def_id();
                let name_trimmed = def_id_name(tcx, def_id, false, true);
                let name = mono_def_id_name(tcx, def_id, List::empty(), false, false);
                graph.add_edge(name, name_trimmed, EdgeType::Trimmed);
            }

            graph
        };

        let path = CacheKind::Static.map(self.path.clone());
        write_to_file(
            graph.serialize(),
            path.clone(),
            |buf| {
                CacheFileDescr::new(
                    crate_name,
                    Some(compile_mode.as_ref()),
                    Some(target.as_ref()),
                    doctest_name.as_deref(),
                    CacheFileKind::Graph,
                )
                .apply(buf);
            },
            false,
        );
    }
}

impl Drop for StaticRTSCallbacks {
    fn drop(&mut self) {
        if self.context.get().is_some() {
            let old_checksums = self.import_checksums(ChecksumKind::Checksum, false);
            let old_checksums_vtbl = self.import_checksums(ChecksumKind::VtblChecksum, false);
            let old_checksums_const = self.import_checksums(ChecksumKind::ConstChecksum, false);

            let context = self.context.get_mut().unwrap();

            context.old_checksums.get_or_init(|| old_checksums);
            context
                .old_checksums_vtbl
                .get_or_init(|| old_checksums_vtbl);
            context
                .old_checksums_const
                .get_or_init(|| old_checksums_const);

            let new_checksums_vtbl = &*NEW_CHECKSUMS_VTBL.get().unwrap().lock().unwrap();
            self.context
                .get()
                .unwrap()
                .new_checksums_vtbl
                .get_or_init(|| new_checksums_vtbl.clone());

            self.export_changes(CacheKind::Static);

            let RTSContext {
                new_checksums,
                new_checksums_vtbl,
                new_checksums_const,
                ..
            } = self.context();

            self.export_checksums(ChecksumKind::Checksum, new_checksums.get().unwrap(), false);
            self.export_checksums(
                ChecksumKind::VtblChecksum,
                new_checksums_vtbl.get().unwrap(),
                false,
            );
            self.export_checksums(
                ChecksumKind::ConstChecksum,
                new_checksums_const.get().unwrap(),
                false,
            );
        } else {
            debug!("Aborting without exporting changes");
        }
    }
}

impl Callbacks for StaticRTSCallbacks {
    fn config(&mut self, config: &mut interface::Config) {
        // The only possibility to intercept vtable entries, which I found, is in their local crate
        config.override_queries = Some(|session, providers| {
            debug!("Modifying providers");

            if std::env::var(ENV_SKIP_ANALYSIS).is_err() {
                OLD_VTABLE_ENTRIES.store(providers.vtable_entries as usize, SeqCst);
                providers.vtable_entries =
                    |tcx, binder| Self::custom_vtable_entries(tcx, binder, SUFFIX_DYN);
            } else {
                trace!("Not analyzing crate {:?}", session.opts.crate_name);
            }
        });
    }

    fn after_analysis<'tcx>(
        &mut self,
        _compiler: &interface::Compiler,
        queries: &'tcx Queries<'tcx>,
    ) -> Compilation {
        if std::env::var(ENV_SKIP_ANALYSIS).is_err() {
            queries.global_ctxt().unwrap().enter(|tcx| {
                let context = self.init_analysis(tcx);
                self.context.get_or_init(|| context);

                self.run_analysis_shared(tcx);
                self.export_graph(tcx);
            });
        }

        Compilation::Continue
    }
}

pub trait GraphAnalysisCallback<'tcx>: AnalysisCallback<'tcx> {
    fn export_graph(&self, tcx: TyCtxt<'tcx>);

    fn create_graph<'arena>(
        &self,
        arena: &'arena Arena<String>,
        tcx: TyCtxt<'tcx>,
    ) -> DependencyGraph<'arena, String> {
        let RTSContext { crate_name, .. } = self.context();

        let graph = create_dependency_graph(tcx, arena, MonoItemCollectionMode::Lazy);
        debug!("Created graph for {}", crate_name);
        graph
    }
}
