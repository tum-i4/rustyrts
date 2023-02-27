use rustc_driver::{Callbacks, Compilation};
use rustc_hir::def_id::LOCAL_CRATE;
use rustc_hir::ConstContext;
use rustc_interface::{interface, Queries};
use rustc_middle::ty::TyCtxt;
use std::fs::read;

use crate::checksums::{get_checksum, Checksums};
use crate::fs_utils::{
    get_changes_path, get_checksums_path, get_graph_path, get_static_path, get_test_path,
    write_to_file,
};
use crate::names::def_id_name;

use super::graph::DependencyGraph;
use super::visitor::GraphVisitor;

pub struct StaticRTSCallbacks {
    graph: DependencyGraph<String>,
    source_path: String,
}

impl Callbacks for StaticRTSCallbacks {
    fn after_analysis<'compiler, 'tcx>(
        &mut self,
        compiler: &'compiler interface::Compiler,
        queries: &'tcx Queries<'tcx>,
    ) -> Compilation {
        queries
            .global_ctxt()
            .unwrap()
            .enter(|tcx| self.run_analysis(compiler, tcx));
        Compilation::Continue
    }
}

impl StaticRTSCallbacks {
    pub fn new(source_path: String) -> Self {
        Self {
            graph: DependencyGraph::new(),
            source_path,
        }
    }

    fn run_analysis(&mut self, _compiler: &interface::Compiler, tcx: TyCtxt) {
        let path_buf = get_static_path(&self.source_path);
        let crate_name = format!("{}", tcx.crate_name(LOCAL_CRATE));
        let crate_id = tcx.sess.local_stable_crate_id().to_u64();

        //##################################################################################################################
        // 1. Import checksums

        let checksums_path_buf = get_checksums_path(path_buf.clone(), &crate_name, crate_id);

        let maybe_checksums = read(checksums_path_buf);

        let old_checksums = {
            if let Ok(checksums) = maybe_checksums {
                Checksums::from(checksums.as_slice())
            } else {
                Checksums::new()
            }
        };

        //##############################################################################################################
        // 2. Visit every def_id that has a MIR body and process traits
        //      1) Creates the graph
        //      2) Computes checksums
        //      3) Calculates which nodes have changed

        let mut graph_visitor = GraphVisitor::new(tcx, &mut self.graph);
        for def_id in tcx.mir_keys(()) {
            graph_visitor.visit(def_id.to_def_id());
        }
        graph_visitor.process_traits();

        //##############################################################################################################
        // 3. Determine which functions represent tests and store the names of those nodes on the filesystem

        let mut tests: Vec<String> = Vec::new();
        for def_id in tcx.mir_keys(()) {
            for attr in tcx.get_attrs_unchecked(def_id.to_def_id()) {
                if attr.name_or_empty().to_ident_string() == "rustc_test_marker" {
                    tests.push(def_id_name(tcx, def_id.to_def_id()));
                }
            }
        }

        if tests.len() > 0 {
            write_to_file(tests.join("\n").to_string(), path_buf.clone(), |buf| {
                get_test_path(buf, &crate_name, crate_id)
            });
        }

        //##############################################################################################################
        // 4. Write checksum mapping and names of changed nodes to filesystem

        let mut new_checksums = Checksums::new();
        let mut changed_nodes = Vec::new();

        for def_id in tcx.mir_keys(()) {
            let has_body = tcx.hir().maybe_body_owned_by(*def_id).is_some();

            if has_body {
                // Apparently optimized_mir() only works in these two cases
                if let Some(ConstContext::ConstFn) | None = tcx.hir().body_const_context(*def_id) {
                    let body = tcx.optimized_mir(*def_id); // 1) See comment above

                    //##########################################################################################################
                    // Check if checksum changed

                    let name = tcx.def_path_debug_str(def_id.to_def_id());
                    let checksum = get_checksum(tcx, body);

                    let maybe_old = old_checksums.inner().get(&name);

                    let changed = match maybe_old {
                        Some(before) => *before != checksum,
                        None => true,
                    };

                    if changed {
                        changed_nodes.push(def_id_name(tcx, def_id.to_def_id()));
                    }
                    new_checksums.inner_mut().insert(name, checksum);
                }
            }
        }

        write_to_file(
            new_checksums.to_string().to_string(),
            path_buf.clone(),
            |buf| get_checksums_path(buf, &crate_name, crate_id),
        );

        write_to_file(
            changed_nodes.join("\n").to_string(),
            path_buf.clone(),
            |buf| get_changes_path(buf, &crate_name, crate_id),
        );

        //##############################################################################################################
        // 5. Write graph to file

        write_to_file(self.graph.to_string(), path_buf.clone(), |buf| {
            get_graph_path(buf, &crate_name, crate_id)
        });
    }
}
