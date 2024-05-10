use std::sync::Mutex;
use std::{mem::transmute, path::PathBuf};

use crate::{
    callbacks_shared::{
        excluded, run_analysis_shared, EXCLUDED, NEW_CHECKSUMS, NEW_CHECKSUMS_CONST,
        NEW_CHECKSUMS_VTBL, OLD_VTABLE_ENTRIES, PATH_BUF,
    },
    fs_utils::get_graph_path,
};
use crate::{
    constants::SUFFIX_DYN,
    static_rts::visitor::{create_dependency_graph, MonoItemCollectionMode},
};
use log::{debug, trace};
use rustc_driver::{Callbacks, Compilation};
use rustc_hir::def_id::LOCAL_CRATE;
use rustc_interface::{interface, Queries};
use rustc_middle::ty::{PolyTraitRef, TyCtxt, VtblEntry};
use rustc_session::config::CrateType;
use std::sync::atomic::Ordering::SeqCst;

use crate::checksums::{get_checksum_vtbl_entry, insert_hashmap, Checksums};
use crate::fs_utils::write_to_file;
use crate::names::def_id_name;

pub struct StaticRTSCallbacks {
    is_compiling_doctests: bool,
}

impl StaticRTSCallbacks {
    pub fn new(maybe_path: Option<PathBuf>, is_compiling_doctests: bool) -> Self {
        if let Some(path) = maybe_path {
            PATH_BUF.get_or_init(|| path);
        }
        Self {
            is_compiling_doctests,
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
            OLD_VTABLE_ENTRIES.store(unsafe { transmute(providers.vtable_entries) }, SeqCst);

            providers.vtable_entries = custom_vtable_entries_monomorphized;
        });

        NEW_CHECKSUMS.get_or_init(|| Mutex::new(Checksums::new()));
        NEW_CHECKSUMS_VTBL.get_or_init(|| Mutex::new(Checksums::new()));
        NEW_CHECKSUMS_CONST.get_or_init(|| Mutex::new(Checksums::new()));
    }

    fn after_analysis<'compiler, 'tcx>(
        &mut self,
        _compiler: &'compiler interface::Compiler,
        queries: &'tcx Queries<'tcx>,
    ) -> Compilation {
        queries.global_ctxt().unwrap().enter(|tcx| {
            if !excluded(|| tcx.crate_name(LOCAL_CRATE).as_str().to_string()) {
                self.run_analysis(tcx);
            }
        });
        Compilation::Continue
    }
}

impl StaticRTSCallbacks {
    fn run_analysis(&mut self, tcx: TyCtxt) {
        if let Some(path) = PATH_BUF.get() {
            let crate_name = format!("{}", tcx.crate_name(LOCAL_CRATE));
            let crate_id = tcx.stable_crate_id(LOCAL_CRATE).as_u64();

            let graph = create_dependency_graph(tcx, MonoItemCollectionMode::Lazy);

            debug!("Created graph for {}", crate_name);

            write_to_file(
                graph.to_string(),
                path.clone(),
                |buf| get_graph_path(buf, &crate_name, crate_id),
                self.is_compiling_doctests,
            );

            run_analysis_shared(tcx, self.is_compiling_doctests, path);
        }
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

    if !excluded(|| tcx.crate_name(LOCAL_CRATE).as_str().to_string()) {
        for entry in result {
            if let VtblEntry::Method(instance) = entry {
                let def_id = instance.def_id();
                if !tcx.is_closure(def_id) && !tcx.is_fn_trait(key.def_id()) {
                    let checksum = get_checksum_vtbl_entry(tcx, &entry);
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
