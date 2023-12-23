use log::trace;
use once_cell::sync::OnceCell;
use rustc_data_structures::sync::Ordering::SeqCst;
use rustc_driver::{Callbacks, Compilation};
use rustc_interface::{interface, Queries};
use rustc_middle::ty::{Instance, PolyFnSig, EarlyBinder, ParamEnv, Visibility};
use rustc_middle::ty::{PolyTraitRef, TyCtxt, VtblEntry};
use rustc_middle::{mir::Body, ty::AssocItem};
use rustc_session::config::CrateType;
use rustc_span::source_map::{FileLoader, RealFileLoader};
use std::mem::transmute;
use std::sync::Mutex;
use std::sync::{atomic::AtomicUsize, RwLock};

use crate::{
    callbacks_shared::{
        excluded, no_instrumentation, run_analysis_shared, EXCLUDED, NEW_CHECKSUMS,
        NEW_CHECKSUMS_CONST, NEW_CHECKSUMS_VTBL, OLD_VTABLE_ENTRIES, PATH_BUF,
    },
    constants::SUFFIX_DYN,
    dynamic_rts::instrumentation::modify_body_dyn,
};

use super::file_loader::{InstrumentationFileLoaderProxy, TestRunnerFileLoaderProxy};
use crate::checksums::{get_checksum_vtbl_entry, insert_hashmap, Checksums};
use crate::dynamic_rts::instrumentation::modify_body;
use crate::fs_utils::get_dynamic_path;
use crate::names::def_id_name;
use bimap::hash::BiHashMap;
use rustc_hir::{
    def_id::{LocalDefId, LOCAL_CRATE, DefId},
    Crate,
};

static OLD_OPTIMIZED_MIR: AtomicUsize = AtomicUsize::new(0);

static VTABLE_ENTRY_SUBSTITUTES: OnceCell<RwLock<BiHashMap<LocalDefId, LocalDefId>>> =
    OnceCell::new();

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
            OLD_OPTIMIZED_MIR.store(unsafe { transmute(providers.optimized_mir) }, SeqCst);
            OLD_ASSOCIATED_ITEM.store(unsafe { transmute(providers.associated_item) }, SeqCst);
            OLD_FN_SIG.store(unsafe { transmute(providers.fn_sig) }, SeqCst);
            OLD_PARAM_ENV.store(unsafe { transmute(providers.param_env) }, SeqCst);
            OLD_VISIBILITY.store(unsafe { transmute(providers.visibility) }, SeqCst);
            OLD_VTABLE_ENTRIES.store(unsafe { transmute(providers.vtable_entries) }, SeqCst);

            providers.optimized_mir = custom_optimized_mir;
            providers.associated_item = custom_associated_item;
            providers.fn_sig = custom_fn_sig;
            providers.param_env = custom_param_env;
            providers.visibility = custom_visibility;
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
    key: LocalDefId,
) -> &'tcx Body<'tcx>{
    if let Some(vtable_substitutes) = VTABLE_ENTRY_SUBSTITUTES.get() {
            if let Some(def_id) = vtable_substitutes.read().unwrap().get_by_left(&key) {
                let result: &Body = tcx.optimized_mir(def_id.to_def_id());

                let ret = if !no_instrumentation(|| tcx.crate_name(LOCAL_CRATE).to_string()) {
                    //##############################################################
                    // 1. Here the MIR is modified to trace this function at runtime
                    let cloned = result.clone();
                    let leaked = Box::leak(Box::new(cloned));

                    modify_body_dyn(tcx, leaked);
                    leaked
                } else {
                    result
                };

                return ret;
            }
    }

    let content = OLD_OPTIMIZED_MIR.load(SeqCst);

    // SAFETY: At this address, the original optimized_mir() function has been stored before.
    // We reinterpret it as a function, while changing the return type to mutable.
    let orig_function = unsafe {
        transmute::<
            usize,
            fn(
                _: TyCtxt<'tcx>,
                _: LocalDefId,
            ) -> &'tcx mut Body<'tcx>, // notice the mutable reference here
        >(content)
    };

    let result = orig_function(tcx, key);

    if !no_instrumentation(|| tcx.crate_name(LOCAL_CRATE).to_string()) {
        //##############################################################
        // 1. Here the MIR is modified to trace this function at runtime

        modify_body(tcx, result);
    }

    result
}

macro_rules! substitute_old_result {
    ($name:ident, $static_name:ident, $custom_name:ident, $in:ty, $out:ty) => {
        static $static_name: AtomicUsize = AtomicUsize::new(0);

        fn $custom_name<'tcx>(tcx: TyCtxt<'tcx>, mut key: $in) -> $out {
            if let Some(local_def) = key.as_local() {
                if let Some(vtable_substitutes) = VTABLE_ENTRY_SUBSTITUTES.get() {
                    if let Some(def_id) = vtable_substitutes.read().unwrap().get_by_left(&local_def)
                    {
                        key = def_id.to_def_id();
                    }
                }
            }

            let content = $static_name.load(SeqCst);

            // SAFETY: At this address, the original $name() function has been stored before.
            // We reinterpret it as a function.
            let orig_function =
                unsafe { transmute::<usize, fn(_: TyCtxt<'tcx>, _: $in) -> $out>(content) };

            orig_function(tcx, key)
        }
    };
}

macro_rules! substitute_old_result_local {
    ($name:ident, $static_name:ident, $custom_name:ident, $in:ty, $out:ty) => {
        static $static_name: AtomicUsize = AtomicUsize::new(0);

        fn $custom_name<'tcx>(tcx: TyCtxt<'tcx>, mut key: $in) -> $out {
                if let Some(vtable_substitutes) = VTABLE_ENTRY_SUBSTITUTES.get() {
                    if let Some(def_id) = vtable_substitutes.read().unwrap().get_by_left(&key)
                    {
                        key = *def_id;
                    }
                }

            let content = $static_name.load(SeqCst);

            // SAFETY: At this address, the original $name() function has been stored before.
            // We reinterpret it as a function.
            let orig_function =
                unsafe { transmute::<usize, fn(_: TyCtxt<'tcx>, _: $in) -> $out>(content) };

            orig_function(tcx, key)
        }
    };
}

substitute_old_result_local!(
    fn_sig,
    OLD_FN_SIG,
    custom_fn_sig,
    LocalDefId,
    EarlyBinder<PolyFnSig<'tcx>>
);

substitute_old_result_local!(
    associated_item,
    OLD_ASSOCIATED_ITEM,
    custom_associated_item,
    LocalDefId,
    AssocItem // Weirdly, we cannot use query_stored::associated_item<'tcx> here
);

substitute_old_result!(
    param_env,
    OLD_PARAM_ENV,
    custom_param_env,
    DefId,
    ParamEnv<'tcx>
);

substitute_old_result_local!(
    visibility,
    OLD_VISIBILITY,
    custom_visibility,
    LocalDefId,
    Visibility<DefId>
);

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
                if !tcx.is_closure(def_id) && !tcx.is_fn_trait(key.def_id()) {
                    // TODO: apply this to static as well

                    let checksum = get_checksum_vtbl_entry(tcx, &entry);
                    let mut name = def_id_name(tcx, def_id, false, true);

                    // 1. Only working for local functions... We prepare duplicating the function
                    // such that we can modify the one that is inserted in to vtable independently.

                    if let Some(local_def) = def_id.as_local() {
                        name += SUFFIX_DYN;

                        let vtable_entry_substitutes =
                            VTABLE_ENTRY_SUBSTITUTES.get_or_init(|| RwLock::new(BiHashMap::new()));

                        let new_def_id = if let Some(new_def_id) = vtable_entry_substitutes
                            .read()
                            .unwrap()
                            .get_by_right(&local_def)
                        {
                            new_def_id.clone()
                        } else {
                            let parent_def = tcx.local_parent(local_def);
                            let span = tcx.def_span(parent_def);

                            // We create a new DefId to duplicate the MIR
                            let def_path = tcx.def_path(def_id).data.pop().unwrap().data;
                            let new_def_id = tcx.at(span).create_def(parent_def, def_path).def_id();

                            // Apparently, we need to duplicate the OwnerInfo as well
                            let krate: &mut Crate =
                                unsafe { std::mem::transmute(tcx.hir_crate(())) };
                            let owner = krate.owners.get(local_def).unwrap();
                            krate.owners.push(*owner);

                            new_def_id
                        };

                        // We can now swap out the function that is put into the vtable
                        let substs = instance.substs;
                        let instance: &mut Instance = unsafe { std::mem::transmute(instance) };
                        *instance = Instance::new(new_def_id.to_def_id(), substs);

                        vtable_entry_substitutes
                            .write()
                            .unwrap()
                            .insert(new_def_id, local_def);
                    }

                    // 2. We add this function to the vtable checksums
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
