use rustc_ast::{
    token::{Delimiter, Token, TokenKind},
    tokenstream::{DelimSpan, Spacing, TokenStream, TokenTree},
    AttrArgs, AttrStyle, Crate, DelimArgs, Path, PathSegment,
};
use rustc_attr::mk_attr;
use rustc_data_structures::sync::Ordering::SeqCst;
use rustc_driver::{Callbacks, Compilation};
use rustc_feature::Features;
use rustc_interface::{interface, Queries};
use rustc_middle::mir::Body;
use rustc_middle::ty::{PolyTraitRef, TyCtxt, VtblEntry};
use rustc_session::config::CrateType;
use rustc_span::{
    source_map::{FileLoader, RealFileLoader},
    sym::{self},
    symbol::Ident,
    Symbol, DUMMY_SP,
};
use std::mem::transmute;

use std::{path::PathBuf, sync::atomic::AtomicUsize};
use tracing::{debug, trace};

use crate::{
    callbacks_shared::{
        excluded, export_changes, export_checksums, import_checksums, init_analysis,
        link_checksums, no_instrumentation, run_analysis_shared, CRATE_ID, CRATE_NAME, EXCLUDED,
        NEW_CHECKSUMS, NEW_CHECKSUMS_CONST, NEW_CHECKSUMS_VTBL, OLD_VTABLE_ENTRIES, PATH_BUF,
        PATH_BUF_DOCTESTS,
    },
    constants::{ENDING_CHECKSUM, ENDING_CHECKSUM_CONST, ENDING_CHECKSUM_VTBL},
    doctest_rts::{self, dynamic::doctests_analysis},
};

use super::file_loader::{InstrumentationFileLoaderProxy, TestRunnerFileLoaderProxy};
use crate::checksums::{get_checksum_vtbl_entry, insert_hashmap};
use crate::dynamic_rts::instrumentation::modify_body;
use crate::names::def_id_name;
use rustc_hir::def_id::{LocalDefId, LOCAL_CRATE};

static OLD_OPTIMIZED_MIR: AtomicUsize = AtomicUsize::new(0);

pub struct DynamicRTSCallbacks {}

impl DynamicRTSCallbacks {
    pub fn new(maybe_path: Option<PathBuf>, maybe_doctest_path: Option<PathBuf>) -> Self {
        if let Some(path) = maybe_path {
            PATH_BUF.get_or_init(|| path);
        }
        if let Some(path) = maybe_doctest_path {
            PATH_BUF_DOCTESTS.get_or_init(|| path);
        }
        Self {}
    }
}

impl Drop for DynamicRTSCallbacks {
    fn drop(&mut self) {
        if let Some(path) = PATH_BUF.get() {
            let Some(crate_name) = CRATE_NAME.get() else {
                return;
            };
            let Some(crate_id) = CRATE_ID.get() else {
                return;
            };
            let crate_id = *crate_id;

            let new_checksums = NEW_CHECKSUMS.get().unwrap().lock().unwrap();
            let new_checksums_vtbl = NEW_CHECKSUMS_VTBL.get().unwrap().lock().unwrap();
            let new_checksums_const = NEW_CHECKSUMS_CONST.get().unwrap().lock().unwrap();

            let old_checksums =
                import_checksums(path.clone(), crate_name, crate_id, ENDING_CHECKSUM);
            let old_checksums_vtbl =
                import_checksums(path.clone(), crate_name, crate_id, ENDING_CHECKSUM_VTBL);
            let old_checksums_const =
                import_checksums(path.clone(), crate_name, crate_id, ENDING_CHECKSUM_CONST);

            export_changes(
                false, // IMPORTANT: static RTS selects based on the old revision
                path.clone(),
                crate_name,
                crate_id,
                &old_checksums,
                &old_checksums_vtbl,
                &old_checksums_const,
                &new_checksums,
                &new_checksums_vtbl,
                &new_checksums_const,
            );
            if let Some(path_doctests) = PATH_BUF_DOCTESTS.get() {
                export_changes(
                    true,
                    path_doctests.clone(),
                    crate_name,
                    crate_id,
                    &old_checksums,
                    &old_checksums_vtbl,
                    &old_checksums_const,
                    &new_checksums,
                    &new_checksums_vtbl,
                    &new_checksums_const,
                );
            }

            export_checksums(
                path.clone(),
                crate_name,
                crate_id,
                &new_checksums,
                &new_checksums_vtbl,
                &new_checksums_const,
                false,
            );
            if let Some(path_doctests) = PATH_BUF_DOCTESTS.get() {
                link_checksums(path.clone(), path_doctests.clone(), crate_name, crate_id);
            }
        }
    }
}

impl Callbacks for DynamicRTSCallbacks {
    fn config(&mut self, config: &mut interface::Config) {
        // There is no point in analyzing a proc macro that is executed a compile time
        if config
            .opts
            .crate_types
            .iter()
            .any(|t| *t == CrateType::ProcMacro)
        {
            debug!(
                "Excluding crate {}",
                config.opts.crate_name.as_ref().unwrap()
            );
            EXCLUDED.get_or_init(|| true);
        }

        let file_loader = if !no_instrumentation(|| {
            config
                .opts
                .crate_name
                .as_ref()
                .cloned()
                .unwrap_or("rustc_out".to_string())
        }) {
            Box::new(TestRunnerFileLoaderProxy {
                delegate: InstrumentationFileLoaderProxy {
                    delegate: RealFileLoader,
                },
            }) as Box<dyn FileLoader + std::marker::Send + std::marker::Sync>
        } else {
            Box::new(RealFileLoader {})
                as Box<dyn FileLoader + std::marker::Send + std::marker::Sync>
        };
        config.file_loader = Some(file_loader);

        // We need to replace this in any case, since we also want to instrument rlib crates
        // Further, the only possibility to intercept vtable entries, which I found, is in their local crate
        config.override_queries = Some(|_session, providers| {
            // SAFETY: We store the addressses of the original functions as a usize.
            OLD_OPTIMIZED_MIR.store(unsafe { transmute(providers.optimized_mir) }, SeqCst);
            OLD_VTABLE_ENTRIES.store(unsafe { transmute(providers.vtable_entries) }, SeqCst);

            providers.optimized_mir = custom_optimized_mir;
            providers.vtable_entries = custom_vtable_entries;
        });
    }

    fn after_crate_root_parsing<'tcx>(
        &mut self,
        _compiler: &interface::Compiler,
        queries: &'tcx Queries<'tcx>,
    ) -> Compilation {
        queries.global_ctxt().unwrap().enter(|tcx| {
            {
                init_analysis(tcx);

                // Inject #![feature(test)] and #![feature(custom_test_runner)] into the inner crate attributes
                let features: &mut Features = unsafe { std::mem::transmute(tcx.features()) };

                features.declared_lib_features.push((sym::test, DUMMY_SP));
                features
                    .declared_lang_features
                    .push((sym::custom_test_frameworks, DUMMY_SP, None));

                features.declared_features.insert(sym::test);
                features
                    .declared_features
                    .insert(sym::custom_test_frameworks);

                features.custom_test_frameworks = true;
            }

            {
                // Add an inner attribute #![test_runner(rustyrts_runner_wrapper)] to the crate attributes
                let borrowed = tcx.crate_for_resolver(()).borrow();
                let krate: &mut Crate = unsafe { std::mem::transmute(&borrowed.0) };

                let generator = &tcx.sess.parse_sess.attr_id_generator;

                {
                    let style = AttrStyle::Inner;
                    let path = Path {
                        span: DUMMY_SP,
                        segments: vec![PathSegment::from_ident(Ident {
                            name: sym::test_runner,
                            span: DUMMY_SP,
                        })]
                        .into(),
                        tokens: None,
                    };

                    let arg_token = Token::new(
                        TokenKind::Ident(Symbol::intern("rustyrts_runner_wrapper"), false),
                        DUMMY_SP,
                    );
                    let arg_tokens =
                        TokenStream::new(vec![TokenTree::Token(arg_token, Spacing::JointHidden)]);
                    let delim_args = DelimArgs {
                        dspan: DelimSpan::dummy(),
                        delim: Delimiter::Parenthesis,
                        tokens: arg_tokens,
                    };
                    let attr_args = AttrArgs::Delimited(delim_args);
                    let span = DUMMY_SP;

                    let attr = mk_attr(generator, style, path, attr_args, span);

                    krate.attrs.push(attr.clone());
                }
            }
            // }
        });

        Compilation::Continue
    }

    fn after_analysis<'compiler, 'tcx>(
        &mut self,
        _compiler: &'compiler interface::Compiler,
        queries: &'tcx Queries<'tcx>,
    ) -> Compilation {
        queries.global_ctxt().unwrap().enter(|tcx| {
            if !excluded(|| tcx.crate_name(LOCAL_CRATE).to_string()) {
                self.run_analysis(tcx)
            }
        });

        Compilation::Continue
    }
}

/// This function is executed instead of optimized_mir() in the compiler
fn custom_optimized_mir<'tcx>(tcx: TyCtxt<'tcx>, key: LocalDefId) -> &'tcx Body<'tcx> {
    let content = OLD_OPTIMIZED_MIR.load(SeqCst);

    // SAFETY: At this address, the original optimized_mir() function has been stored before.
    // We reinterpret it as a function, while changing the return type to mutable.
    let orig_function = unsafe {
        transmute::<
            usize,
            fn(_: TyCtxt<'tcx>, _: LocalDefId) -> &'tcx mut Body<'tcx>, // notice the mutable reference here
        >(content)
    };

    let result = orig_function(tcx, key);

    if !no_instrumentation(|| tcx.crate_name(LOCAL_CRATE).to_string()) {
        //##############################################################
        // 1. Here the MIR is modified to debug this function at runtime

        modify_body(tcx, result);
    }

    result
}

impl DynamicRTSCallbacks {
    fn run_analysis(&mut self, tcx: TyCtxt) {
        run_analysis_shared(tcx);
        if let Some(_path) = PATH_BUF_DOCTESTS.get() {
            doctest_rts::dynamic::doctests_analysis(tcx);
        }
    }
}

fn custom_vtable_entries<'tcx>(
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

    if !excluded(|| tcx.crate_name(LOCAL_CRATE).to_string()) {
        for entry in result {
            if let VtblEntry::Method(instance) = entry {
                let def_id = instance.def_id();
                if !tcx.is_closure(def_id) && !tcx.is_fn_trait(key.def_id()) {
                    let checksum = get_checksum_vtbl_entry(tcx, &entry);
                    let name = def_id_name(tcx, def_id, false, true);

                    trace!("Considering {:?} in checksums of {}", instance, name);

                    insert_hashmap(
                        &mut *NEW_CHECKSUMS_VTBL.get().unwrap().lock().unwrap(),
                        &name,
                        checksum,
                    );
                }
            }
        }
    }

    result
}
