use log::debug;
use rustc_driver::{Callbacks, Compilation};
use rustc_hir::def_id::LOCAL_CRATE;
use rustc_hir::ConstContext;
use rustc_interface::{interface, Queries};
use rustc_middle::ty::TyCtxt;

use crate::callbacks_shared::{excluded, prepare_analysis, run_analysis_shared};
use crate::checksums::{get_checksum, insert_hashmap, Checksums};
use crate::fs_utils::{get_graph_path, get_static_path, write_to_file};
use crate::names::def_id_name;

use super::graph::DependencyGraph;
use super::visitor::GraphVisitor;

pub struct StaticRTSCallbacks {
    graph: DependencyGraph<String>,
    target_path: String,
}

impl Callbacks for StaticRTSCallbacks {
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
    pub fn new(target_path: String) -> Self {
        Self {
            graph: DependencyGraph::new(),
            target_path,
        }
    }

    fn run_analysis(&mut self, tcx: TyCtxt) {
        if !excluded(tcx) {
            let path_buf = get_static_path(&self.target_path);
            let crate_name = format!("{}", tcx.crate_name(LOCAL_CRATE));
            let crate_id = tcx.sess.local_stable_crate_id().to_u64();

            prepare_analysis(path_buf.clone());

            //##############################################################################################################
            // 1. Visit every def_id that has a MIR body and process traits
            //      1) Creates the graph
            //      2) Write graph to file

            let mut graph_visitor = GraphVisitor::new(tcx, &mut self.graph);
            for def_id in tcx.mir_keys(()) {
                graph_visitor.visit(def_id.to_def_id());
            }
            graph_visitor.process_traits();

            write_to_file(
                self.graph.to_string(),
                path_buf.clone(),
                |buf| get_graph_path(buf, &crate_name, crate_id),
                false,
            );

            debug!("Generated dependency graph for {}", crate_name);

            //##############################################################################################################
            // 2. Calculate checksum of every MIR body

            let mut new_checksums = Checksums::new();
            let mut new_checksums_ctfe = Checksums::new();

            for def_id in tcx.mir_keys(()) {
                let has_body = tcx.hir().maybe_body_owned_by(*def_id).is_some();

                if has_body {
                    match tcx.hir().body_const_context(*def_id) {
                        Some(ConstContext::ConstFn) | None => {
                            let body = tcx.optimized_mir(*def_id);
                            let name = def_id_name(tcx, def_id.to_def_id());
                            let checksum = get_checksum(tcx, body);

                            insert_hashmap(new_checksums.inner_mut(), name, checksum)
                        }
                        Some(ConstContext::Static(..)) | Some(ConstContext::Const) => {
                            let body = tcx.mir_for_ctfe(*def_id);
                            let name = def_id_name(tcx, def_id.to_def_id());
                            let checksum = get_checksum(tcx, body);

                            insert_hashmap(new_checksums_ctfe.inner_mut(), name, checksum)
                        }
                    };
                }
            }

            //##############################################################################################################
            // 2. Determine which functions represent tests and store the names of those nodes on the filesystem
            // 3. Import checksums
            // 4. Calculate new checksums and names of changed nodes and write this information to the filesystem
            run_analysis_shared(tcx, path_buf, new_checksums, new_checksums_ctfe);
        }
    }
}
