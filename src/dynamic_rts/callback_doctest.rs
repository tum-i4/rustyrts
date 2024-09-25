use std::path::PathBuf;

use crate::{
    callbacks_shared::{
        AnalysisCallback, ChecksumsCallback, RTSContext, DOCTEST_PREFIX, ENTRY_FN,
        NEW_CHECKSUMS_VTBL, OLD_VTABLE_ENTRIES,
    },
    constants::{ENV_SKIP_ANALYSIS, ENV_SKIP_INSTRUMENTATION},
    dynamic_rts::{
        callback::{InstrumentingCallback, OLD_OPTIMIZED_MIR},
        mir_util::Traceable,
    },
    fs_utils::ChecksumKind,
    names::{def_id_name, IS_COMPILING_DOCTESTS},
};
use once_cell::sync::OnceCell;
use rustc_data_structures::sync::Ordering::SeqCst;
use rustc_driver_impl::{Callbacks, Compilation};
use rustc_interface::{interface::Compiler, Config, Queries};
use tracing::{debug, trace};

pub struct InstrumentingDoctestRTSCallbacks {}

impl InstrumentingDoctestRTSCallbacks {
    pub fn new() -> Self {
        IS_COMPILING_DOCTESTS.store(true, SeqCst);
        Self {}
    }
}

impl InstrumentingCallback for InstrumentingDoctestRTSCallbacks {
    fn modify_body<'tcx>(
        tcx: rustc_middle::ty::TyCtxt<'tcx>,
        body: &mut rustc_middle::mir::Body<'tcx>,
    ) {
        let _prof_timer = tcx.prof.generic_activity("RUSTYRTS_instrumentation");

        let def_id = body.source.instance.def_id();
        let outer = def_id_name(tcx, def_id, false, true);

        trace!("Visiting {}", outer);

        let mut cache_ret = None;

        if let Some(entry_def) = ENTRY_FN.get_or_init(|| tcx.entry_fn(()).map(|(def_id, _)| def_id))
        {
            if def_id == *entry_def {
                // IMPORTANT: The order in which insert_post, trace, insert_pre are called is critical here
                // 1. insert_post, 2. trace, 3. insert_pre

                let doctest_name = std::env::var("UNSTABLE_RUSTDOC_TEST_PATH")
                    .expect("Did not find doctest name")
                    .chars()
                    .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
                    .collect::<String>();
                let fn_name = DOCTEST_PREFIX.to_string() + &doctest_name;

                body.insert_post_test(tcx, &doctest_name, &mut cache_ret, true);
                body.insert_trace(tcx, &fn_name, &mut cache_ret);
                body.insert_pre_test(tcx, &doctest_name, &mut cache_ret, true);
                return;
            }
        }

        body.insert_trace(tcx, &outer, &mut cache_ret);

        #[cfg(unix)]
        body.check_calls_to_exit(tcx, &mut cache_ret);

        // RATIONALE: Function pre_main is empty and therefore not codegened
        // Leaving this in, in case it gets (re-)populated
        //
        // #[cfg(unix)]
        // if let Some(entry_def) = ENTRY_FN.get().unwrap() {
        //     if def_id == *entry_def {
        //         body.insert_pre_main(tcx, &mut cache_ret);
        //     }
        // }
    }
}

impl Callbacks for InstrumentingDoctestRTSCallbacks {
    fn config(&mut self, config: &mut Config) {
        // No need to inject test runner
        // Doctests are executed separately

        // We need to replace this in any case, since we also want to instrument rlib crates
        // Further, the only possibility to intercept vtable entries, which I found, is in their local crate
        config.override_queries = Some(|session, providers| {
            debug!("Modifying providers");

            if std::env::var(ENV_SKIP_INSTRUMENTATION).is_err() {
                OLD_OPTIMIZED_MIR.store(providers.optimized_mir as usize, SeqCst);
                providers.optimized_mir = Self::custom_optimized_mir;
            } else {
                trace!("Not instrumenting crate {:?}", session.opts.crate_name);
            }
        });
    }
}

pub struct AnalyzingRTSCallbacks {
    path: PathBuf,
    context: OnceCell<RTSContext>,
}

impl AnalyzingRTSCallbacks {
    pub fn new(target_dir: PathBuf) -> Self {
        IS_COMPILING_DOCTESTS.store(true, SeqCst);
        Self {
            path: target_dir,
            context: OnceCell::new(),
        }
    }
}

impl<'tcx> AnalysisCallback<'tcx> for AnalyzingRTSCallbacks {}

impl ChecksumsCallback for AnalyzingRTSCallbacks {
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

impl Drop for AnalyzingRTSCallbacks {
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

impl Callbacks for AnalyzingRTSCallbacks {
    fn config(&mut self, config: &mut Config) {
        // The only possibility to intercept vtable entries, which I found, is in their local crate
        config.override_queries = Some(|session, providers| {
            debug!("Modifying providers");

            if std::env::var(ENV_SKIP_ANALYSIS).is_err() {
                OLD_VTABLE_ENTRIES.store(providers.vtable_entries as usize, SeqCst);
                providers.vtable_entries =
                    |tcx, binder| Self::custom_vtable_entries(tcx, binder, "");
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
            });
        }

        Compilation::Stop
    }
}
