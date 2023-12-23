use log::trace;
use rustc_data_structures::sync::Ordering::SeqCst;
use rustc_driver::{Callbacks, Compilation};
use rustc_interface::{interface, Queries};
use rustc_middle::ty::query::{query_keys, query_stored};
use rustc_middle::ty::{PolyTraitRef, TyCtxt, VtblEntry};
use rustc_session::config::CrateType;
use rustc_span::source_map::{FileLoader, RealFileLoader};
use std::mem::transmute;
use std::sync::atomic::AtomicUsize;
use std::sync::Mutex;

use crate::callbacks_shared::{
    excluded, no_instrumentation, run_analysis_shared, EXCLUDED, NEW_CHECKSUMS,
    NEW_CHECKSUMS_CONST, NEW_CHECKSUMS_VTBL, OLD_VTABLE_ENTRIES,
};

use crate::checksums::{get_checksum_vtbl_entry, insert_hashmap, Checksums};
use crate::dynamic_rts::instrumentation::modify_body;
use crate::fs_utils::get_dynamic_path;
use crate::names::def_id_name;
use crate::static_rts::callback::PATH_BUF;
use rustc_hir::def_id::LOCAL_CRATE;

use super::file_loader::{InstrumentationFileLoaderProxy, TestRunnerFileLoaderProxy};

static OLD_OPTIMIZED_MIR_PTR: AtomicUsize = AtomicUsize::new(0);

pub struct DynamicRTSCallbacks {}

impl DynamicRTSCallbacks {
    pub fn new() -> Self {
        PATH_BUF.get_or_init(|| get_dynamic_path(true, None));
        Self {}
    }
}

impl Callbacks for DynamicRTSCallbacks {
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

        let file_loader =
            if !no_instrumentation(|| config.opts.crate_name.as_ref().unwrap().to_string()) {
                Box::new(TestRunnerFileLoaderProxy {
                    delegate: InstrumentationFileLoaderProxy {
                        delegate: RealFileLoader,
                    },
                }) as Box<dyn FileLoader + std::marker::Send + std::marker::Sync>
            } else {
                Box::new(RealFileLoader {})
                    as Box<dyn FileLoader + std::marker::Send + std::marker::Sync>
            };
        config.file_loader = Some(file_loader);

        if !excluded(|| config.opts.crate_name.as_ref().unwrap().to_string()) {
            NEW_CHECKSUMS.get_or_init(|| Mutex::new(Checksums::new()));
            NEW_CHECKSUMS_VTBL.get_or_init(|| Mutex::new(Checksums::new()));
            NEW_CHECKSUMS_CONST.get_or_init(|| Mutex::new(Checksums::new()));
        }

        // We need to replace this in any case, since we also want to instrument rlib crates
        // Further, the only possibility to intercept vtable entries, which I found, is in their local crate
        config.override_queries = Some(|_session, providers, _extern_providers| {
            // SAFETY: We store the address of the original optimized_mir function as a usize.
            OLD_OPTIMIZED_MIR_PTR.store(unsafe { transmute(providers.optimized_mir) }, SeqCst);
            OLD_VTABLE_ENTRIES.store(unsafe { transmute(providers.vtable_entries) }, SeqCst);

            providers.optimized_mir = custom_optimized_mir;
            providers.vtable_entries = custom_vtable_entries;
        });
    }

    fn after_analysis<'compiler, 'tcx>(
        &mut self,
        _compiler: &'compiler interface::Compiler,
        queries: &'tcx Queries<'tcx>,
    ) -> Compilation {
        queries.global_ctxt().unwrap().enter(|tcx| {
            if !excluded(|| tcx.crate_name(LOCAL_CRATE).to_string()) {
                self.run_analysis(tcx)
            }
        });

        Compilation::Continue
    }
}

/// This function is executed instead of optimized_mir() in the compiler
fn custom_optimized_mir<'tcx>(
    tcx: TyCtxt<'tcx>,
    def: query_keys::optimized_mir<'tcx>,
) -> query_stored::optimized_mir<'tcx> {
    let content = OLD_OPTIMIZED_MIR_PTR.load(SeqCst);

    // SAFETY: At this address, the original optimized_mir() function has been stored before.
    // We reinterpret it as a function, while changing the return type to mutable.
    let orig_function = unsafe {
        transmute::<
            usize,
            fn(
                _: TyCtxt<'tcx>,
                _: query_keys::optimized_mir<'tcx>,
            ) -> &'tcx mut rustc_middle::mir::Body<'tcx>, // notice the mutable reference here
        >(content)
    };

    let result = orig_function(tcx, def);

    if !no_instrumentation(|| tcx.crate_name(LOCAL_CRATE).to_string()) {
        //##############################################################
        // 1. Here the MIR is modified to trace this function at runtime

        modify_body(tcx, result);
    }

    result
}

impl DynamicRTSCallbacks {
    fn run_analysis(&mut self, tcx: TyCtxt) {
        run_analysis_shared(tcx);
    }
}

fn custom_vtable_entries<'tcx>(
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

    if !excluded(|| tcx.crate_name(LOCAL_CRATE).to_string()) {
        for entry in result {
            if let VtblEntry::Method(instance) = entry {
                let def_id = instance.def_id();
                if !tcx.is_closure(def_id) {
                    let name = def_id_name(tcx, def_id, false, true);
                    let checksum = get_checksum_vtbl_entry(tcx, &entry);
                    trace!("Considering {:?} in checksums of {}", instance, name);

                    insert_hashmap(
                        &mut *NEW_CHECKSUMS_VTBL.get().unwrap().lock().unwrap(),
                        &name,
                        checksum,
                    )
                }

                // let mut instance: &mut Instance = unsafe { std::mem::transmute::<_, _>(instance) };
                // let trace_dyn_fn_def_id = get_def_id_trace_dyn_fn(tcx).unwrap();

                // instance.def = InstanceDef::Item(WithOptConstParam {
                //     did: trace_dyn_fn_def_id,
                //     const_param_did: Some(def_id),
                // });
            }
        }
    }

    result
}
