use std::fs::{read, File};
use std::io::Write;
use std::path::PathBuf;

use rustc_driver::{Callbacks, Compilation};
use rustc_hir::def_id::LOCAL_CRATE;
use rustc_interface::{interface, Queries};
use rustc_middle::ty::TyCtxt;

use crate::analysis::visitor::GraphVisitor;
use crate::graph::graph::DependencyGraph;
use crate::paths::{
    get_base_path, get_changes_path, get_checksums_path, get_graph_path, get_test_path,
};

use super::checksums::Checksums;
use super::util::{def_id_name, load_tcx};

pub struct RustyRTSCallbacks {
    graph: DependencyGraph<String>,
    source_path: String,
}

impl Callbacks for RustyRTSCallbacks {
    fn after_analysis<'compiler, 'tcx>(
        &mut self,
        compiler: &'compiler interface::Compiler,
        queries: &'tcx Queries<'tcx>,
    ) -> Compilation {
        load_tcx(queries, |tcx| self.run_analysis(compiler, tcx));
        Compilation::Continue
    }
}

impl RustyRTSCallbacks {
    pub fn new(source_path: String) -> Self {
        Self {
            graph: DependencyGraph::new(),
            source_path,
        }
    }

    fn run_analysis(&mut self, _compiler: &interface::Compiler, tcx: TyCtxt) {
        let path_buf = get_base_path(&self.source_path);
        let crate_name = format!("{}", tcx.crate_name(LOCAL_CRATE));
        let crate_id = tcx.sess.local_stable_crate_id().to_u64();

        //##################################################################################################################
        // Import checksums

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
        // 1. Visit every def_id that has a MIR body and process traits
        //      1) Creates the graph
        //      2) Computes checksums
        //      3) Calculates which nodes have changed

        let mut visitor = GraphVisitor::new(tcx, &mut self.graph, old_checksums);
        for def_id in tcx.mir_keys(()) {
            visitor.visit(def_id.to_def_id());
        }
        visitor.process_traits();

        //##############################################################################################################
        // 2. Determine which functions represent tests and store the names of those nodes on the filesystem

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
        // 3. Write checksum mapping and names of changed nodes to filesystem

        let (checksums, changed_nodes) = visitor.finalize();

        write_to_file(checksums.to_string().to_string(), path_buf.clone(), |buf| {
            get_checksums_path(buf, &crate_name, crate_id)
        });

        write_to_file(
            changed_nodes.join("\n").to_string(),
            path_buf.clone(),
            |buf| get_changes_path(buf, &crate_name, crate_id),
        );

        //##############################################################################################################
        // Write graph to file

        write_to_file(self.graph.to_string(), path_buf.clone(), |buf| {
            get_graph_path(buf, &crate_name, crate_id)
        });
    }
}

/// Computes the location of a file from a closure
/// and overwrites the content of this file
///
/// ## Arguments
/// * `content` - new content of the file
/// * `path_buf` - `PathBuf` that points to the parent directory
/// * `initializer` - function that modifies path_buf - candidates: `get_graph_path`, `get_test_path`, `get_changes_path`
///
fn write_to_file<F>(content: String, path_buf: PathBuf, initializer: F)
where
    F: FnOnce(PathBuf) -> PathBuf,
{
    let path_buf = initializer(path_buf);
    let mut file = match File::create(path_buf.as_path()) {
        Ok(file) => file,
        Err(reason) => panic!("Failed to create file: {}", reason),
    };

    match file.write_all(content.as_bytes()) {
        Ok(_) => {}
        Err(reason) => panic!("Failed to write to file: {}", reason),
    };
}
