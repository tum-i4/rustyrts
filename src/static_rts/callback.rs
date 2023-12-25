use std::mem::transmute;
use std::sync::atomic::AtomicUsize;
use std::sync::Mutex;

use crate::{
    callbacks_shared::{
        excluded, run_analysis_shared, EXCLUDED, NEW_CHECKSUMS, NEW_CHECKSUMS_CONST,
        NEW_CHECKSUMS_VTBL, OLD_VTABLE_ENTRIES, PATH_BUF, TEST_MARKER,
    },
    static_rts::visitor::ResolvingVisitor,
};
use crate::{constants::SUFFIX_DYN, fs_utils::get_dependencies_path};
use itertools::Itertools;
use log::trace;
use rustc_driver::{Callbacks, Compilation};
use rustc_hir::{def_id::{LOCAL_CRATE, LocalDefId}, AttributeMap};
use rustc_interface::{interface, Queries};
use rustc_middle::{ty::{PolyTraitRef, TyCtxt, VtblEntry}, mir::Body};
use rustc_session::config::CrateType;
use std::sync::atomic::Ordering::SeqCst;

use crate::checksums::{get_checksum_vtbl_entry, insert_hashmap, Checksums};
use crate::fs_utils::{get_static_path, write_to_file};
use crate::names::def_id_name;

static OLD_OPTIMIZED_MIR_PTR: AtomicUsize = AtomicUsize::new(0);

pub struct StaticRTSCallbacks {}

impl StaticRTSCallbacks {
    pub fn new() -> Self {
        PATH_BUF.get_or_init(|| get_static_path(true));
        Self {}
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
        config.override_queries = Some(|_session, providers, _extern_providers| {
            // SAFETY: We store the address of the original vtable_entries function as a usize.
            OLD_VTABLE_ENTRIES.store(unsafe { transmute(providers.vtable_entries) }, SeqCst);
            OLD_OPTIMIZED_MIR_PTR.store(unsafe { transmute(providers.optimized_mir) }, SeqCst);

            providers.vtable_entries = custom_vtable_entries_monomorphized;
            providers.optimized_mir = custom_optimized_mir;
        });

        if !excluded(|| config.opts.crate_name.as_ref().unwrap().to_string()) {
            NEW_CHECKSUMS.get_or_init(|| Mutex::new(Checksums::new()));
            NEW_CHECKSUMS_VTBL.get_or_init(|| Mutex::new(Checksums::new()));
            NEW_CHECKSUMS_CONST.get_or_init(|| Mutex::new(Checksums::new()));
        }
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

/// This function is executed instead of optimized_mir() in the compiler
fn custom_optimized_mir<'tcx>(
    tcx: TyCtxt<'tcx>,
    def: LocalDefId,
) -> &'tcx Body<'tcx> {
    let content = OLD_OPTIMIZED_MIR_PTR.load(SeqCst);

    // SAFETY: At this address, the original optimized_mir() function has been stored before.
    // We reinterpret it as a function, while changing the return type to mutable.
    let orig_function = unsafe {
        transmute::<
            usize,
            fn(
                _: TyCtxt<'tcx>,
                _: LocalDefId,
            ) -> &'tcx mut rustc_middle::mir::Body<'tcx>, // notice the mutable reference here
        >(content)
    };

    let body = orig_function(tcx, def);

    let name = def_id_name(tcx, def.to_def_id(), true, false);
    let attrs = &tcx.hir_crate(()).owners
        [tcx.local_def_id_to_hir_id(def).owner.def_id]
        .as_owner()
        .map_or(AttributeMap::EMPTY, |o| &o.attrs)
        .map;

    let is_test = attrs
        .iter()
        .flat_map(|(_, list)| list.iter())
        .unique_by(|i| i.id)
        .any(|attr| attr.name_or_empty().to_ident_string() == TEST_MARKER);

    if is_test {
        let dependencies = ResolvingVisitor::find_dependencies(tcx, body)
            .into_iter()
            .fold(String::new(), |mut acc, node| {
                acc.push_str(&node);
                acc.push_str("\n");
                acc
            });
        write_to_file(
            dependencies,
            PATH_BUF.get().unwrap().clone(),
            |p| get_dependencies_path(p, &name[0..name.len() - 13]),
            false,
        );
        trace!("Collected dependencies for {}", name);
    }
    body
}

impl StaticRTSCallbacks {
    fn run_analysis(&mut self, tcx: TyCtxt) {
        run_analysis_shared(tcx);
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
                    if let Some(trait_fn) = tcx.impl_of_method(def_id).and_then(|impl_def| {
                        tcx.impl_trait_ref(impl_def).and_then(|trait_def| {
                            let implementors = tcx.impl_item_implementor_ids(impl_def);

                            let associated_items = tcx.associated_item_def_ids(trait_def.skip_binder().def_id);
                            for item in associated_items {
                                if implementors.get(item).is_some_and(|impl_fn|*impl_fn == def_id) {
                                    return Some(item);
                                }
                            }                           
                            return None;
                        })
                    }) {
                        let name = def_id_name(tcx, *trait_fn, false, true).to_owned() + SUFFIX_DYN;
                        let checksum = get_checksum_vtbl_entry(tcx, &entry);

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
    }

    result
}
