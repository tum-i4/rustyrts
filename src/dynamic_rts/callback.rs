use once_cell::sync::OnceCell;
use rustc_data_structures::sync::Ordering::SeqCst;
use rustc_driver::{Callbacks, Compilation};
use rustc_hir::ConstContext;
use rustc_interface::{interface, Queries};
use rustc_middle::ty::query::{query_keys, query_stored};
use rustc_middle::{mir::visit::MutVisitor, ty::TyCtxt};
use rustc_span::source_map::{FileLoader, RealFileLoader};
use std::mem::transmute;
use std::sync::atomic::{AtomicBool, AtomicUsize};
use std::sync::Mutex;

use crate::callbacks_shared::{excluded, insert_hashmap, prepare_analysis, run_analysis_shared};
use crate::checksums::{get_checksum, Checksums};
use crate::fs_utils::get_dynamic_path;
use crate::names::def_id_name;

use super::visitor::MirManipulatorVisitor;

static mut NEW_CHECKSUMS: OnceCell<Mutex<Checksums>> = OnceCell::new();
static mut NEW_CHECKSUMS_CTFE: OnceCell<Mutex<Checksums>> = OnceCell::new();

static OLD_OPTIMIZED_MIR_PTR: AtomicUsize = AtomicUsize::new(0);
static OLD_MIR_FOR_CTFE_PTR: AtomicUsize = AtomicUsize::new(0);
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
            OLD_OPTIMIZED_MIR_PTR.store(unsafe { transmute(providers.optimized_mir) }, SeqCst);
            OLD_MIR_FOR_CTFE_PTR.store(unsafe { transmute(providers.mir_for_ctfe) }, SeqCst);

            providers.optimized_mir = custom_optimized_mir;
            providers.mir_for_ctfe = custom_mir_for_ctfe;
        });

        let path_buf = get_dynamic_path(&self.source_path);
        prepare_analysis(path_buf.clone());
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

    let mut result = orig_function(tcx, def);

    if !excluded(tcx) {
        //##############################################################
        // 1. We compute the checksum before modifying the MIR

        let name = def_id_name(tcx, result.source.def_id()).expect_one();
        let checksum = get_checksum(tcx, result);

        let new_checksums = unsafe { NEW_CHECKSUMS.get_or_init(|| Mutex::new(Checksums::new())) };

        {
            let mut handle = new_checksums.lock().unwrap();
            insert_hashmap(handle.inner_mut(), name, checksum);
        }

        //##############################################################
        // 2. Here the MIR is modified to trace this function at runtime

        let mut visitor = MirManipulatorVisitor::new(tcx);
        visitor.visit_body(&mut result);
    }

    result
}

/// This function is executed instead of mir_for_ctfe() in the compiler
fn custom_mir_for_ctfe<'tcx>(
    tcx: TyCtxt<'tcx>,
    def: query_keys::mir_for_ctfe<'tcx>,
) -> query_stored::mir_for_ctfe<'tcx> {
    let content = OLD_MIR_FOR_CTFE_PTR.load(SeqCst);

    // SAFETY: At this address, the original mir_for_ctfe() function has been stored before.
    // We reinterpret it as a function.
    let orig_function = unsafe {
        transmute::<
            usize,
            fn(
                _: TyCtxt<'tcx>,
                _: query_keys::mir_for_ctfe<'tcx>,
            ) -> query_stored::mir_for_ctfe<'tcx>, // notice the mutable reference here
        >(content)
    };

    let result = orig_function(tcx, def);

    if !excluded(tcx) {
        //##############################################################
        // 1. We compute the checksum

        let name = def_id_name(tcx, result.source.def_id()).expect_one();
        let checksum = get_checksum(tcx, result);

        let new_checksums =
            unsafe { NEW_CHECKSUMS_CTFE.get_or_init(|| Mutex::new(Checksums::new())) };

        {
            let mut handle = new_checksums.lock().unwrap();
            insert_hashmap(handle.inner_mut(), name, checksum);
        }
    }

    result
}

impl DynamicRTSCallbacks {
    fn run_analysis(&mut self, tcx: TyCtxt) {
        if !excluded(tcx) {
            let path_buf = get_dynamic_path(&self.source_path);

            //##############################################################################################################
            // 1. Invoke optimized_mir or mir_for_ctfe for every MIR body, to compute checksums

            for def_id in tcx.mir_keys(()) {
                let has_body = tcx.hir().maybe_body_owned_by(*def_id).is_some();

                if has_body {
                    let _body = match tcx.hir().body_const_context(*def_id) {
                        Some(ConstContext::ConstFn) | None => tcx.optimized_mir(*def_id),
                        Some(ConstContext::Static(..)) | Some(ConstContext::Const) => {
                            tcx.mir_for_ctfe(*def_id)
                        }
                    };
                }
            }

            //##############################################################################################################
            // 2. Determine which functions represent tests and store the names of those nodes on the filesystem
            // 3. Import old checksums
            // 4. Determine names of changed nodes and write this information to the filesystem
            run_analysis_shared(
                tcx,
                path_buf,
                unsafe { NEW_CHECKSUMS.take() }
                    .map(|mutex| mutex.into_inner().unwrap())
                    .unwrap_or_else(|| Checksums::new()),
                unsafe { NEW_CHECKSUMS_CTFE.take() }
                    .map(|mutex| mutex.into_inner().unwrap())
                    .unwrap_or_else(|| Checksums::new()),
            );
        }
    }
}
