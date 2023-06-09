use itertools::Itertools;
use log::{debug, trace};
use rustc_data_structures::sync::Ordering::SeqCst;
use rustc_driver::{Callbacks, Compilation};
use rustc_interface::{interface, Queries};
use rustc_middle::mir::mono::MonoItem;
use rustc_middle::ty::query::{query_keys, query_stored};
use rustc_middle::ty::{PolyTraitRef, TyCtxt, VtblEntry};
use rustc_span::source_map::{FileLoader, RealFileLoader};
use std::collections::HashSet;
use std::mem::transmute;
use std::sync::atomic::{AtomicBool, AtomicUsize};
use std::sync::Mutex;

use crate::callbacks_shared::{
    excluded, run_analysis_shared, NEW_CHECKSUMS, NEW_CHECKSUMS_CONST, NEW_CHECKSUMS_VTBL, NODES,
    OLD_VTABLE_ENTRIES,
};

use crate::checksums::{get_checksum_body, get_checksum_vtbl_entry, insert_hashmap, Checksums};
use crate::const_visitor::ConstVisitor;
use crate::dynamic_rts::instrumentation::modify_body;
use crate::fs_utils::get_dynamic_path;
use crate::names::def_id_name;
use crate::static_rts::callback::PATH_BUF;

static OLD_OPTIMIZED_MIR_PTR: AtomicUsize = AtomicUsize::new(0);

static EXTERN_CRATE_INSERTED: AtomicBool = AtomicBool::new(false);

pub struct FileLoaderProxy {
    delegate: RealFileLoader,
}

impl FileLoader for FileLoaderProxy {
    fn file_exists(&self, path: &std::path::Path) -> bool {
        self.delegate.file_exists(path)
    }

    fn read_file(&self, path: &std::path::Path) -> std::io::Result<String> {
        let content = self.delegate.read_file(path)?;
        if !EXTERN_CRATE_INSERTED.load(SeqCst) {
            EXTERN_CRATE_INSERTED.store(true, SeqCst);

            if content.contains("#![feature(custom_test_frameworks)]") {
                panic!("Dynamic RustyRTS does not support using a custom test framework. Please use static RustyRTS instead");
            }

            let content = content.replace("#![feature(test)]", "");
            let extended_content = format!(
                "#![feature(test)]
                #![feature(custom_test_frameworks)]
                #![test_runner(rustyrts_runner_wrapper)]
                
                {}

                #[allow(unused_extern_crates)]
                extern crate rustyrts_dynamic_rlib;

                #[allow(unused_extern_crates)]
                extern crate test as rustyrts_test;
                
                #[link(name = \"rustyrts_dynamic_runner\")]
                #[allow(improper_ctypes)]
                #[allow(dead_code)]
                extern {{
                    fn rustyrts_runner(tests: &[&rustyrts_test::TestDescAndFn]);
                }}
                
                #[allow(dead_code)]
                fn rustyrts_runner_wrapper(tests: &[&rustyrts_test::TestDescAndFn]) 
                {{ 
                    unsafe {{ rustyrts_runner(tests); }}
                }}",
                content
            )
            .to_string();

            Ok(extended_content)
        } else {
            Ok(content)
        }
    }
}

pub struct DynamicRTSCallbacks {}

impl DynamicRTSCallbacks {
    pub fn new() -> Self {
        PATH_BUF.get_or_init(|| get_dynamic_path(true));
        Self {}
    }
}

impl Callbacks for DynamicRTSCallbacks {
    fn config(&mut self, config: &mut interface::Config) {
        let file_loader = FileLoaderProxy {
            delegate: RealFileLoader,
        };
        config.file_loader = Some(Box::new(file_loader));

        config.override_queries = Some(|_session, providers, _extern_providers| {
            // SAFETY: We store the address of the original optimized_mir function as a usize.
            OLD_OPTIMIZED_MIR_PTR.store(unsafe { transmute(providers.optimized_mir) }, SeqCst);
            OLD_VTABLE_ENTRIES.store(unsafe { transmute(providers.vtable_entries) }, SeqCst);

            providers.optimized_mir = custom_optimized_mir;
            providers.vtable_entries = custom_vtable_entries;
        });

        NODES.get_or_init(|| Mutex::new(HashSet::new()));
        NEW_CHECKSUMS.get_or_init(|| Mutex::new(Checksums::new()));
        NEW_CHECKSUMS_VTBL.get_or_init(|| Mutex::new(Checksums::new()));
        NEW_CHECKSUMS_CONST.get_or_init(|| Mutex::new(Checksums::new()));
    }

    fn after_analysis<'compiler, 'tcx>(
        &mut self,
        _compiler: &'compiler interface::Compiler,
        queries: &'tcx Queries<'tcx>,
    ) -> Compilation {
        queries
            .global_ctxt()
            .unwrap()
            .enter(|tcx| self.run_analysis(tcx));

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

    if !excluded(tcx) {
        //##############################################################
        // 1. We compute the checksum before modifying the MIR
        let def_id = result.source.def_id();

        let name = def_id_name(tcx, def_id, &[]);
        let checksum = get_checksum_body(tcx, result);

        trace!("Inserting checksum of {}", name);

        {
            let mut new_checksums = NEW_CHECKSUMS.get().unwrap().lock().unwrap();
            insert_hashmap(&mut *new_checksums, &name, checksum);

            for body in tcx.promoted_mir(def_id) {
                let checksum = get_checksum_body(tcx, body);
                insert_hashmap(&mut *new_checksums, &name, checksum);
            }
        }

        //##############################################################
        // 2. Here the MIR is modified to trace this function at runtime

        modify_body(tcx, result);
    }

    result
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

    for entry in result {
        if let VtblEntry::Method(instance) = entry {
            let def_id = instance.def_id();

            let name = def_id_name(tcx, def_id, &[]);
            let checksum = get_checksum_vtbl_entry(tcx, &entry);
            debug!("Considering {:?} in checksums of {}", instance, name);

            insert_hashmap(
                &mut *NEW_CHECKSUMS_VTBL.get().unwrap().lock().unwrap(),
                &name,
                checksum,
            )
        }
    }

    result
}

impl DynamicRTSCallbacks {
    fn run_analysis(&mut self, tcx: TyCtxt) {
        if !excluded(tcx) {
            //##########################################################################################################
            // 1. Collect the names of all mir bodies (because optimized mir is not always invoked)

            let nodes = NODES.get().unwrap();

            let code_gen_units = tcx.collect_and_partition_mono_items(()).1;
            let bodies = code_gen_units
                .iter()
                .flat_map(|c| c.items().keys())
                .filter(|m| if let MonoItem::Fn(_) = m { true } else { false })
                .map(|m| {
                    let MonoItem::Fn(instance) = m else {unreachable!()};
                    instance
                })
                .filter(|i| tcx.is_mir_available(i.def_id()))
                .filter(|i| i.def_id().is_local()) // TODO: Check if this is feasible
                .map(|i| (tcx.optimized_mir(i.def_id()), i.substs))
                .collect_vec();

            //##########################################################################################################
            // 1. Visit every instance (pair of MIR body and corresponding generic args)
            //    and every body from const
            //      1) Creates the graph
            //      2) Write graph to file

            let mut const_visitor = ConstVisitor::new(tcx);
            for (body, substs) in bodies {
                const_visitor.visit(&body, substs);

                let def_id = body.source.def_id();
                let name: String = def_id_name(tcx, def_id, &[]);
                nodes.lock().unwrap().insert(name);
                let _body = tcx.optimized_mir(def_id);
            }

            //##########################################################################################################
            // 2. Determine which functions represent tests and store the names of those nodes on the filesystem
            // 3. Import old checksums
            // 4. Determine names of changed nodes and write this information to the filesystem
            run_analysis_shared(tcx);
        }
    }
}
