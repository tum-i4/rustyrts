use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering::SeqCst;
use std::{mem::transmute, sync::Mutex};

use rustc_driver::{Callbacks, Compilation};
use rustc_interface::{interface, Queries};
use rustc_middle::ty::TyCtxt;

use rustc_data_structures::steal::Steal;
use rustc_middle::ty::query::query_keys::mir_built;

use crate::analysis::visitor::GraphVisitor;

pub struct RustyRTSCallbacks {
    pub source_name: String,
}

impl RustyRTSCallbacks {
    pub fn new() -> Self {
        Self {
            source_name: String::new(),
        }
    }

    fn run_analysis(&self, _compiler: &interface::Compiler, tcx: TyCtxt) {
        let mut res = VISITOR_MUTEX.lock().unwrap();
        *res = false;
        let mut visitor = GraphVisitor::new(tcx);
        visitor.visit();
        *res = true;
    }
}

static OLD_FUNCTION_PTR: AtomicU64 = AtomicU64::new(0);
static VISITOR_MUTEX: Mutex<bool> = Mutex::new(true);

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
    let def_kind = tcx.def_kind(def_id);

    if let Ok(_) = VISITOR_MUTEX.try_lock() {
        println!(
            "Built MIR of {:?} {}",
            def_kind,
            tcx.def_path_debug_str(def_id)
        );
    }

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
        config.opts.unstable_opts.incremental_ignore_spans = true;
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
