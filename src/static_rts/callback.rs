use std::mem::transmute;
use std::path::PathBuf;
use std::sync::Mutex;

use itertools::Itertools;
use log::debug;
use once_cell::sync::OnceCell;
use rustc_data_structures::sync::Ordering::SeqCst;
use rustc_driver::{Callbacks, Compilation};
use rustc_hir::def_id::LOCAL_CRATE;
use rustc_hir::ConstContext;
use rustc_interface::{interface, Queries};
use rustc_middle::mir::mono::MonoItem;
use rustc_middle::ty::{List, PolyTraitRef, TyCtxt, VtblEntry};

use crate::callbacks_shared::{
    excluded, run_analysis_shared, NEW_CHECKSUMS, NEW_CHECKSUMS_CONST, NEW_CHECKSUMS_VTBL, NODES,
    OLD_VTABLE_ENTRIES,
};

use crate::checksums::{get_checksum_body, get_checksum_vtbl_entry, insert_hashmap, Checksums};
use crate::const_visitor::ConstVisitor;
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
        config.override_queries = Some(|_session, providers, _extern_providers| {
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
        queries
            .global_ctxt()
            .unwrap()
            .enter(|tcx| self.run_analysis(tcx));
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
        if !excluded(tcx) {
            let crate_name = format!("{}", tcx.crate_name(LOCAL_CRATE));
            let crate_id = tcx.sess.local_stable_crate_id().to_u64();

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
                .filter(|i| i.def_id().is_local())
                .map(|i| (tcx.optimized_mir(i.def_id()), i.substs))
                .collect_vec();

            //##########################################################################################################
            // 1. Visit every instance (pair of MIR body and corresponding generic args)
            //    and every body from const
            //      1) Creates the graph
            //      2) Write graph to file

            let mut graph_visitor = GraphVisitor::new(tcx, &mut self.graph);
            let mut const_visitor = ConstVisitor::new(tcx);
            for (body, substs) in bodies {
                graph_visitor.visit(&body, substs);
                if body.source.def_id().is_local() {
                    const_visitor.visit(&body, substs);
                }
            }

            write_to_file(
                self.graph.to_string(),
                PATH_BUF.get().unwrap().clone(),
                |buf| get_graph_path(buf, &crate_name, crate_id),
                false,
            );

            debug!("Generated dependency graph for {}", crate_name);

            //##########################################################################################################
            // 2. Calculate checksum of every MIR body

            let mut new_checksums = NEW_CHECKSUMS.get().unwrap().lock().unwrap();

            for def_id in tcx.mir_keys(()) {
                match tcx.hir().body_const_context(*def_id) {
                    Some(ConstContext::ConstFn) | None => {
                        let body = tcx.optimized_mir(*def_id);
                        let name = def_id_name(tcx, def_id.to_def_id(), &[]);

                        let checksum = get_checksum_body(tcx, body);
                        insert_hashmap(&mut *new_checksums, &name, checksum);

                        for body in tcx.promoted_mir(*def_id) {
                            let checksum = get_checksum_body(tcx, body);
                            insert_hashmap(&mut *new_checksums, &name, checksum);
                        }
                    }
                    _ => {}
                };
            }

            //##########################################################################################################
            // 2. Determine which functions represent tests and store the names of those nodes on the filesystem
            // 3. Import checksums
            // 4. Calculate new checksums and names of changed nodes and write this information to the filesystem

            NODES.get_or_init(|| Mutex::new(new_checksums.keys().map(|s| s.clone()).collect()));
            run_analysis_shared(tcx);
        }
    }
}

pub(crate) fn custom_vtable_entries_monomorphized<'tcx>(
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
            let substs = if cfg!(feature = "monomorphize_all") {
                instance.substs.as_slice()
            } else {
                List::empty()
            };

            let name = def_id_name(tcx, instance.def_id(), substs);

            let checksum = get_checksum_vtbl_entry(tcx, &entry);

            insert_hashmap(
                &mut *NEW_CHECKSUMS_VTBL.get().unwrap().lock().unwrap(),
                &name,
                checksum,
            )
        }
    }

    result
}
