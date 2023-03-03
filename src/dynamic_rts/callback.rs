use regex::Regex;
use rustc_data_structures::sync::Ordering::SeqCst;
use rustc_driver::{Callbacks, Compilation};
use rustc_interface::{interface, Queries};
use rustc_middle::ty::query::{query_keys, query_stored};
use rustc_middle::{mir::visit::MutVisitor, ty::TyCtxt};
use rustc_span::source_map::{FileLoader, RealFileLoader};
use std::mem::transmute;
use std::sync::atomic::{AtomicBool, AtomicUsize};

use crate::callbacks_shared::run_analysis_shared;
use crate::fs_utils::get_dynamic_path;

use super::visitor::MirManipulatorVisitor;

static OLD_FUNCTION_PTR: AtomicUsize = AtomicUsize::new(0);
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

            let extended_content = format!(
                "#![feature(test)]
                #![feature(custom_test_frameworks)]
                #![test_runner(rustyrts_runner_wrapper)]
                
                {}

                extern crate rustyrts_dynamic_rlib;
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

            let forbid_regex = Regex::new(r"#!\[.*?forbid\(.*?\)\]").unwrap();
            let filtered_content = forbid_regex.replace_all(&extended_content, "").to_string();
            Ok(filtered_content)
        } else {
            Ok(content)
        }
    }
}

pub struct DynamicRTSCallbacks {
    source_path: String,
}

impl DynamicRTSCallbacks {
    pub fn new(source_path: String) -> Self {
        Self { source_path }
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
            OLD_FUNCTION_PTR.store(unsafe { transmute(providers.optimized_mir) }, SeqCst);

            providers.optimized_mir = custom_optimized_mir;
        });
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
    let content = OLD_FUNCTION_PTR.load(SeqCst);

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

    let mut result = orig_function(tcx, def);

    //##############################################################
    // 1. Here the MIR is modified to trace this function at runtime

    let mut visitor = MirManipulatorVisitor::new(tcx);
    visitor.visit_body(&mut result);
    result
}

impl DynamicRTSCallbacks {
    fn run_analysis(&mut self, tcx: TyCtxt) {
        let path_buf = get_dynamic_path(&self.source_path);

        //##############################################################################################################
        // 2. Determine which functions represent tests and store the names of those nodes on the filesystem
        // 3. Import checksums
        // 4. Calculate new checksums and names of changed nodes and write this information to the filesystem
        run_analysis_shared(tcx, path_buf);
    }
}
