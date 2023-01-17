use std::fs::{read_to_string, File};
use std::io::Write;
use std::path::PathBuf;
use std::sync::RwLock;

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
use super::util::def_path_debug_str_custom;

static BASE_PATH: RwLock<String> = RwLock::new(String::new());

pub struct RustyRTSCallbacks {
    graph: DependencyGraph<String>,
}

impl RustyRTSCallbacks {
    pub fn new(source_path: String) -> Self {
        let mut handle = BASE_PATH.write().unwrap();
        *handle = source_path;

        Self {
            graph: DependencyGraph::new(),
        }
    }

    fn run_analysis(&mut self, _compiler: &interface::Compiler, tcx: TyCtxt) {
        let crate_name = format!("{}", tcx.crate_name(LOCAL_CRATE));
        let crate_id = tcx.sess.local_stable_crate_id().to_u64();

        let old_checksums = self.import_checksums(tcx);

        let mut visitor = GraphVisitor::new(tcx, &mut self.graph, old_checksums);

        //##############################################################################################################
        // Visit every def_id

        for def_id in tcx.mir_keys(()) {
            visitor.visit(def_id.to_def_id());
        }
        visitor.process_traits();

        //##############################################################################################################
        // Determine which functions represent tests

        let mut tests: Vec<String> = Vec::new();
        for def_id in tcx.mir_keys(()) {
            for attr in tcx.get_attrs_unchecked(def_id.to_def_id()) {
                if attr.name_or_empty().to_ident_string() == "rustc_test_marker" {
                    tests.push(def_path_debug_str_custom(tcx, def_id.to_def_id()));
                }
            }
        }

        if tests.len() > 0 {
            write_to_file(tests.join("\n").to_string(), |buf| {
                get_test_path(buf, &crate_name, crate_id)
            });
        }

        //##############################################################################################################
        // Write new checksums and changed nodes to file

        let (checksums, changed_nodes) = visitor.terminate();
        self.export_checksums(tcx, &checksums);

        write_to_file(checksums.to_string().to_string(), |buf| {
            get_checksums_path(buf, &crate_name, crate_id)
        });

        write_to_file(changed_nodes.join("\n").to_string(), |buf| {
            get_changes_path(buf, &crate_name, crate_id)
        });

        //##############################################################################################################
        // Write graph to file

        write_to_file(self.graph.to_string(), |buf| {
            get_graph_path(buf, &crate_name, crate_id)
        });
    }

    fn export_checksums(&self, tcx: TyCtxt, checksums: &Checksums) {
        let crate_name = format!("{}", tcx.crate_name(LOCAL_CRATE));
        let crate_id = tcx.sess.local_stable_crate_id().to_u64();

        write_to_file(checksums.to_string(), |buf| {
            get_checksums_path(buf, &crate_name, crate_id)
        });
    }

    fn import_checksums(&self, tcx: TyCtxt) -> Checksums {
        //##################################################################################################################
        // Import checksums

        let crate_name = format!("{}", tcx.crate_name(LOCAL_CRATE));
        let crate_id = tcx.sess.local_stable_crate_id().to_u64();

        let handle = BASE_PATH.read().unwrap();
        let path_buf = get_base_path(&*handle);
        let checksums_path_buf = get_checksums_path(path_buf.clone(), &crate_name, crate_id);

        let maybe = read_to_string(checksums_path_buf);

        if let Ok(checksums_str) = maybe {
            checksums_str.parse().expect("Failed to parse checksums")
        } else {
            Checksums::new()
        }
    }
}

impl Callbacks for RustyRTSCallbacks {
    /// Called after analysis. Return value instructs the compiler whether to
    /// continue the compilation afterwards (defaults to `Compilation::Continue`)
    fn after_analysis<'compiler, 'tcx>(
        &mut self,
        compiler: &'compiler interface::Compiler,
        queries: &'tcx Queries<'tcx>,
    ) -> Compilation {
        load_ctxt(queries, |tcx| self.run_analysis(compiler, tcx));
        Compilation::Continue
    }
}

#[rustversion::since(1.68.0)]
fn load_ctxt<'tcx, F: FnOnce(TyCtxt<'tcx>)>(queries: &'tcx Queries<'tcx>, f: F) {
    queries.global_ctxt().unwrap().enter(|tcx| f(tcx));
}

#[rustversion::before(1.68.0)]
fn load_ctxt(queries: &Queries) {
    queries
        .global_ctxt()
        .unwrap()
        .peek_mut() // Apparently peek_mut() has been removed since version 1.68.0
        .enter(|tcx| f(tcx));
}

fn write_to_file<F>(content: String, path_buf_init: F)
where
    F: FnOnce(PathBuf) -> PathBuf,
{
    let handle = BASE_PATH.read().unwrap();
    let path_buf = get_base_path(&*handle).clone();

    let path_buf = path_buf_init(path_buf);
    let mut file = match File::create(path_buf.as_path()) {
        Ok(file) => file,
        Err(reason) => panic!("Failed to create file: {}", reason),
    };

    match file.write_all(content.as_bytes()) {
        Ok(_) => {}
        Err(reason) => panic!("Failed to write to file: {}", reason),
    };
}
