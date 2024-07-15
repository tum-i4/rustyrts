use crate::{
    callbacks_shared::{
        AnalysisCallback, ChecksumsCallback, RTSContext, ENTRY_FN, NEW_CHECKSUMS_VTBL,
        OLD_VTABLE_ENTRIES,
    },
    constants::{ENV_SKIP_ANALYSIS, SUFFIX_DYN},
    fs_utils::{append_to_file, CacheFileDescr, CacheFileKind, CacheKind, ChecksumKind},
    names::{def_id_name, IS_COMPILING_DOCTESTS},
    static_rts::callback::GraphAnalysisCallback,
};
use once_cell::sync::OnceCell;
use rustc_data_structures::sync::Ordering::SeqCst;
use rustc_driver_impl::{Callbacks, Compilation};
use rustc_interface::{interface::Compiler, Config, Queries};
use rustc_middle::ty::TyCtxt;
use std::path::PathBuf;
use tracing::{debug, trace};

use super::graph::{serialize::ArenaSerializable, DependencyGraph, EdgeType};

pub struct StaticDoctestRTSCallbacks {
    path: PathBuf,
    context: OnceCell<RTSContext>,
}

impl StaticDoctestRTSCallbacks {
    pub fn new(target_dir: PathBuf) -> Self {
        IS_COMPILING_DOCTESTS.store(true, SeqCst);
        Self {
            path: target_dir,
            context: OnceCell::new(),
        }
    }
}

impl<'tcx> AnalysisCallback<'tcx> for StaticDoctestRTSCallbacks {}

impl ChecksumsCallback for StaticDoctestRTSCallbacks {
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

impl<'tcx> GraphAnalysisCallback<'tcx> for StaticDoctestRTSCallbacks {
    fn export_graph(&self, tcx: TyCtxt<'tcx>) {
        let RTSContext {
            crate_name,
            compile_mode,
            doctest_name,
            ..
        } = self.context();

        let arena = internment::Arena::new();

        let graph = {
            let _prof_timer = tcx
                .prof
                .generic_activity("RUSTYRTS_dependency_graph_creation");

            let mut graph: DependencyGraph<'_, String> = self.create_graph(&arena, tcx);

            let entry_def = ENTRY_FN
                .get_or_init(|| tcx.entry_fn(()).map(|(def_id, _)| def_id))
                .unwrap();
            let entry_name = def_id_name(tcx, entry_def, false, true);

            graph.add_edge(
                doctest_name.as_ref().unwrap().to_string(),
                entry_name,
                EdgeType::Trimmed,
            );

            graph
        };

        let path = CacheKind::Static.map(self.path.clone());
        append_to_file(
            // IMPORTANT: requires filesystem locking, since multiple threads write to this file in parallel
            graph.serialize(),
            path.clone(),
            |buf| {
                CacheFileDescr::new(
                    crate_name,
                    Some(compile_mode.as_ref()),
                    doctest_name.as_deref(),
                    CacheFileKind::Graph,
                )
                .apply(buf);
            },
        );
    }
}

impl Drop for StaticDoctestRTSCallbacks {
    fn drop(&mut self) {
        if self.context.get().is_some() {
            let new_checksums_vtbl = &*NEW_CHECKSUMS_VTBL.get().unwrap().lock().unwrap();
            self.context
                .get()
                .unwrap()
                .new_checksums_vtbl
                .get_or_init(|| new_checksums_vtbl.clone());

            let RTSContext {
                new_checksums,
                new_checksums_vtbl,
                new_checksums_const,
                ..
            } = self.context();

            self.export_checksums(ChecksumKind::Checksum, new_checksums.get().unwrap(), true);
            self.export_checksums(
                ChecksumKind::VtblChecksum,
                new_checksums_vtbl.get().unwrap(),
                true,
            );
            self.export_checksums(
                ChecksumKind::ConstChecksum,
                new_checksums_const.get().unwrap(),
                true,
            );
        }
    }
}

impl Callbacks for StaticDoctestRTSCallbacks {
    fn config(&mut self, config: &mut Config) {
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

    fn after_crate_root_parsing<'tcx>(
        &mut self,
        _compiler: &Compiler,
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

        Compilation::Stop
    }
}
