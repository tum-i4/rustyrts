use std::fs::{File, OpenOptions};
use std::io::Write;
use std::mem::transmute;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::RwLock;

use rustc_driver::{Callbacks, Compilation};
use rustc_hir::def_id::LOCAL_CRATE;
use rustc_interface::{interface, Queries};
use rustc_middle::ty::TyCtxt;

use rustc_data_structures::steal::Steal;
use rustc_middle::ty::query::query_keys::mir_built;

use crate::analysis::visitor::GraphVisitor;
use crate::graph::graph::DependencyGraph;
use crate::paths::{get_base_path, get_changes_path, get_graph_path, get_test_path};

use super::util::def_path_debug_str_custom;

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
        let handle = BASE_PATH.read().unwrap();
        let path_buf = get_base_path(&*handle);

        let crate_name = format!("{}", tcx.crate_name(LOCAL_CRATE));
        let crate_id = tcx.sess.local_stable_crate_id().to_u64();

        //##############################################################################################################
        // Create Graph

        let mut visitor = GraphVisitor::new(tcx, &mut self.graph);

        for def_id in tcx.mir_keys(()) {
            visitor.visit(def_id.to_def_id());
        }
        visitor.process_traits();

        let graph_path_buf = get_graph_path(path_buf.clone(), &crate_name, crate_id);

        let mut file = match File::create(graph_path_buf.as_path()) {
            Ok(file) => file,
            Err(reason) => panic!("Failed to create file: {}", reason),
        };

        match file.write_all(self.graph.to_string().as_bytes()) {
            Ok(_) => {}
            Err(reason) => panic!("Failed to write to file: {}", reason),
        };

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
            let tests_path_buf = get_test_path(path_buf.clone(), &crate_name, crate_id);

            let mut file = match File::create(tests_path_buf.as_path()) {
                Ok(file) => file,
                Err(reason) => panic!("Failed to create file: {}", reason),
            };

            match file.write_all(tests.join("\n").as_bytes()) {
                Ok(_) => {}
                Err(reason) => panic!("Failed to write to file: {}", reason),
            };
        }
    }
}

static OLD_FUNCTION_PTR: AtomicU64 = AtomicU64::new(0);
static BASE_PATH: RwLock<String> = RwLock::new(String::new());

fn custom_mir_built<'tcx>(
    tcx: TyCtxt<'tcx>,
    def: mir_built<'tcx>,
) -> &'tcx Steal<rustc_middle::mir::Body<'tcx>> {
    let content = OLD_FUNCTION_PTR.load(SeqCst);
    let old_function = unsafe {
        transmute::<
            u64,
            fn(_: TyCtxt<'tcx>, _: mir_built<'tcx>) -> &'tcx Steal<rustc_middle::mir::Body<'tcx>>,
        >(content)
    };

    let result = old_function(tcx, def);

    let def_id = result.borrow().source.instance.def_id();
    //let def_kind = tcx.def_kind(def_id);

    //##################################################################################################################
    // Append names of changed nodes to file

    let crate_name = format!("{}", tcx.crate_name(LOCAL_CRATE));
    let crate_id = tcx.sess.local_stable_crate_id().to_u64();

    let handle = BASE_PATH.read().unwrap();
    let path_buf = get_base_path(&*handle);
    let changes_path_buf = get_changes_path(path_buf.clone(), &crate_name, crate_id);

    let mut file = match OpenOptions::new()
        .create(true)
        .write(true)
        .append(true)
        .open(changes_path_buf.as_path())
    {
        Ok(file) => file,
        Err(reason) => panic!("Failed to open file: {}", reason),
    };

    match file.write_all(format!("{}\n", def_path_debug_str_custom(tcx, def_id)).as_bytes()) {
        Ok(_) => {}
        Err(reason) => panic!("Failed to write to file: {}", reason),
    };

    return result;
}

impl Callbacks for RustyRTSCallbacks {
    /// Called before creating the compiler instance
    fn config(&mut self, config: &mut interface::Config) {
        config.override_queries = Some(|_sess, providers, _external_providers| {
            // inject extended custum mir_build query
            let old_mir_built = providers.mir_built;
            OLD_FUNCTION_PTR.store(old_mir_built as u64, SeqCst);
            providers.mir_built = custom_mir_built;
        });

        // set incremental_ignore_spans to true
        // config.opts.unstable_opts.incremental_ignore_spans = true;
    }

    /// Called after analysis. Return value instructs the compiler whether to
    /// continue the compilation afterwards (defaults to `Compilation::Continue`)
    fn after_analysis<'compiler, 'tcx>(
        &mut self,
        compiler: &'compiler interface::Compiler,
        queries: &'tcx Queries<'tcx>,
    ) -> Compilation {
        queries
            .global_ctxt()
            .unwrap()
            .peek_mut()
            .enter(|tcx| self.run_analysis(compiler, tcx));
        Compilation::Continue
    }
}
