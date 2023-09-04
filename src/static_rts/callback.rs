use std::mem::transmute;
use std::path::PathBuf;
use std::sync::Mutex;

use crate::callbacks_shared::{
    excluded, run_analysis_shared, EXCLUDED, NEW_CHECKSUMS, NEW_CHECKSUMS_CONST,
    NEW_CHECKSUMS_VTBL, OLD_VTABLE_ENTRIES,
};
use crate::constants::SUFFIX_DYN;
use itertools::Itertools;
use log::trace;
use once_cell::sync::OnceCell;
use rustc_driver::{Callbacks, Compilation};
use rustc_hir::def_id::LOCAL_CRATE;
use rustc_interface::{interface, Queries};
use rustc_middle::mir::mono::MonoItem;
use rustc_middle::ty::{List, PolyTraitRef, TyCtxt, VtblEntry};
use rustc_session::config::CrateType;
use std::sync::atomic::Ordering::SeqCst;

use crate::checksums::{get_checksum_vtbl_entry, insert_hashmap, Checksums};
use crate::fs_utils::{get_graph_path, get_static_path, write_to_file};
use crate::names::def_id_name;

use super::graph::DependencyGraph;
use super::visitor::GraphVisitor;

pub(crate) static PATH_BUF: OnceCell<PathBuf> = OnceCell::new();

pub struct StaticRTSCallbacks {
    graph: DependencyGraph<String>,
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

        // The only possibility to intercept vtable entries, which I found, is in their local crate
        config.override_queries = Some(|_session, providers, _extern_providers| {
            // SAFETY: We store the address of the original vtable_entries function as a usize.
            OLD_VTABLE_ENTRIES.store(unsafe { transmute(providers.vtable_entries) }, SeqCst);

            providers.vtable_entries = custom_vtable_entries_monomorphized;
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

impl StaticRTSCallbacks {
    pub fn new() -> Self {
        PATH_BUF.get_or_init(|| get_static_path(true));
        Self {
            graph: DependencyGraph::new(),
        }
    }

    fn run_analysis(&mut self, tcx: TyCtxt) {
        let crate_name = format!("{}", tcx.crate_name(LOCAL_CRATE));
        let crate_id = tcx.sess.local_stable_crate_id().to_u64();

        //##############################################################################################################
        // Collect all MIR bodies that are relevant for code generation

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
            .filter(|i| tcx.is_codegened_item(i.def_id()))
            //.filter(|i| i.def_id().is_local()) // It is not feasible to only analyze local MIR
            .map(|i| (tcx.optimized_mir(i.def_id()), i.substs))
            .collect_vec();

        //##############################################################################################################
        // 1. Visit every instance (pair of MIR body and corresponding generic args)
        //    and every body from const
        //      1) Creates the graph
        //      2) Write graph to file

        let mut graph_visitor = GraphVisitor::new(tcx, &mut self.graph);
        for (body, substs) in &bodies {
            graph_visitor.visit(&body, substs);
        }

        write_to_file(
            self.graph.to_string(),
            PATH_BUF.get().unwrap().clone(),
            |buf| get_graph_path(buf, &crate_name, crate_id),
            false,
        );

        trace!("Generated dependency graph for {}", crate_name);

        //##############################################################################################################
        // Continue at shared analysis

        run_analysis_shared(tcx, bodies);
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
                let substs = if cfg!(not(feature = "monomorphize")) {
                    List::empty()
                } else {
                    instance.substs
                };

                // TODO: it should be feasible to exclude closures here

                let name = def_id_name(tcx, instance.def_id(), substs, false, true).to_owned()
                    + SUFFIX_DYN;

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

    result
}
