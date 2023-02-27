use crate::checksums::{get_checksum, Checksums};
use crate::fs_utils::{
    get_changes_path, get_checksums_path, get_dynamic_path, get_test_path, write_to_file,
};
use crate::names::def_id_name;

use super::visitor::MirManipulatorVisitor;
use rustc_data_structures::sync::Ordering::SeqCst;
use rustc_driver::{Callbacks, Compilation};
use rustc_hir::def_id::LOCAL_CRATE;
use rustc_hir::ConstContext;
use rustc_interface::{interface, Queries};
use rustc_middle::ty::query::query_keys::optimized_mir;
use rustc_middle::{mir::visit::MutVisitor, ty::TyCtxt};
use rustc_span::source_map::{FileLoader, RealFileLoader};
use std::fs::read;
use std::mem::transmute;
use std::sync::atomic::{AtomicBool, AtomicUsize};

static OLD_FUNCTION_PTR: AtomicUsize = AtomicUsize::new(0);
static EXTERN_CRATE_INSERTED: AtomicBool = AtomicBool::new(false);

pub struct DynamicRTSCallbacks {
    source_path: String,
}

impl DynamicRTSCallbacks {
    pub fn new(source_path: String) -> Self {
        Self { source_path }
    }
}

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
            let extended_content =
                format!("{}\nextern crate rustyrts_dynamic_rlib;", content).to_string();
            Ok(extended_content)
        } else {
            Ok(content)
        }
    }
}

/// This function is executed instead of optimized_mir() in the compiler
fn custom_optimized_mir<'tcx>(
    tcx: TyCtxt<'tcx>,
    def: optimized_mir<'tcx>,
) -> &'tcx rustc_middle::mir::Body<'tcx> {
    let content = OLD_FUNCTION_PTR.load(SeqCst);
    let old_function = unsafe {
        transmute::<
            usize,
            fn(_: TyCtxt<'tcx>, _: optimized_mir<'tcx>) -> &'tcx mut rustc_middle::mir::Body<'tcx>,
        >(content)
    };

    let mut result = old_function(tcx, def);

    let Some(mut visitor) = MirManipulatorVisitor::try_new(tcx) else {panic!("Did not find rlib crate")};
    visitor.visit_body(&mut result);
    result
}

impl Callbacks for DynamicRTSCallbacks {
    fn config(&mut self, config: &mut interface::Config) {
        let file_loader = FileLoaderProxy {
            delegate: RealFileLoader,
        };
        config.file_loader = Some(Box::new(file_loader));

        config.override_queries = Some(|_session, providers, _extern_providers| {
            OLD_FUNCTION_PTR.store(unsafe { transmute(providers.optimized_mir) }, SeqCst);

            providers.optimized_mir = custom_optimized_mir;
        });
    }

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

impl DynamicRTSCallbacks {
    fn run_analysis(&mut self, _compiler: &interface::Compiler, tcx: TyCtxt) {
        let path_buf = get_dynamic_path(&self.source_path);
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
    }
}
