use super::file_loader::{InstrumentationFileLoaderProxy, TestRunnerFileLoaderProxy};
use crate::{callbacks_shared::TEST_MARKER, dynamic_rts::mir_util::Traceable, names::def_id_name};
use crate::{
    callbacks_shared::{
        AnalysisCallback, ChecksumsCallback, RTSContext, NEW_CHECKSUMS_VTBL, OLD_VTABLE_ENTRIES,
    },
    constants::{ENV_SKIP_ANALYSIS, ENV_SKIP_INSTRUMENTATION},
    fs_utils::{CacheKind, ChecksumKind},
};
use once_cell::sync::OnceCell;
use rustc_ast::{
    token::{Delimiter, Token, TokenKind},
    tokenstream::{DelimSpan, Spacing, TokenStream, TokenTree},
    AttrArgs, AttrStyle, Crate, DelimArgs, Path, PathSegment,
};
use rustc_attr::mk_attr;
use rustc_data_structures::sync::Ordering::SeqCst;
use rustc_driver::{Callbacks, Compilation};
use rustc_feature::Features;
use rustc_hir::def_id::DefId;
use rustc_hir::{def_id::LocalDefId, AttributeMap};
use rustc_interface::{interface, Config, Queries};
use rustc_middle::mir::Body;
use rustc_middle::ty::TyCtxt;
use rustc_span::{
    source_map::{FileLoader, RealFileLoader},
    sym::{self},
    symbol::Ident,
    Symbol, DUMMY_SP,
};
use std::mem::transmute;
use std::{path::PathBuf, sync::atomic::AtomicUsize};
use tracing::{debug, trace};

pub static OLD_OPTIMIZED_MIR: AtomicUsize = AtomicUsize::new(0);

pub struct InstrumentingRTSCallbacks {}

impl InstrumentingRTSCallbacks {
    pub fn new() -> Self {
        Self {}
    }
}

impl InstrumentingCallback for InstrumentingRTSCallbacks {
    fn modify_body<'tcx>(tcx: TyCtxt<'tcx>, body: &mut Body<'tcx>) {
        let def_id = body.source.instance.def_id();
        let outer = def_id_name(tcx, def_id, false, true);

        trace!("Visiting {}", outer);

        let mut cache_ret = None;

        let attrs = &tcx.hir_crate(()).owners[tcx
            .local_def_id_to_hir_id(def_id.expect_local())
            .owner
            .def_id]
            .as_owner()
            .map_or(AttributeMap::EMPTY, |o| &o.attrs)
            .map;

        for (_, list) in attrs.iter() {
            for attr in *list {
                if attr.name_or_empty().to_ident_string() == TEST_MARKER {
                    let def_path = def_id_name(tcx, def_id, false, false);
                    let def_path_test = &def_path[0..def_path.len() - 13];

                    // IMPORTANT: The order in which insert_post, insert_pre are called is critical here
                    // 1. insert_post 2. insert_pre

                    body.insert_post_test(tcx, def_path_test, &mut cache_ret, &mut None, false);
                    body.insert_pre_test(tcx, &mut cache_ret);
                    return;
                }
            }
        }

        #[cfg(unix)]
        if let Some(entry_def) = ENTRY_FN.get_or_init(|| tcx.entry_fn(()).map(|(def_id, _)| def_id))
        {
            if def_id == *entry_def {
                // IMPORTANT: The order in which insert_post, trace, insert_pre are called is critical here
                // 1. insert_post, 2. trace, 3. insert_pre

                body.insert_post_main(tcx, &mut cache_ret, &mut None);
            }
        }

        body.insert_trace(tcx, &outer, &mut cache_ret);

        #[cfg(unix)]
        body.check_calls_to_exit(tcx, &mut cache_ret);

        #[cfg(unix)]
        if let Some(entry_def) = ENTRY_FN.get().unwrap() {
            if def_id == *entry_def {
                body.insert_pre_main(tcx, &mut cache_ret);
            }
        }
    }
}

impl Callbacks for InstrumentingRTSCallbacks {
    fn config(&mut self, config: &mut Config) {
        if std::env::var(ENV_SKIP_INSTRUMENTATION).is_err() {
            let file_loader = Box::new(InstrumentationFileLoaderProxy {
                delegate: RealFileLoader,
            })
                as Box<dyn FileLoader + std::marker::Send + std::marker::Sync>;
            config.file_loader = Some(file_loader);
        }

        // We need to replace this in any case, since we also want to instrument rlib crates
        // Further, the only possibility to intercept vtable entries, which I found, is in their local crate
        config.override_queries = Some(|session, providers| {
            debug!("Modifying providers");

            if std::env::var(ENV_SKIP_INSTRUMENTATION).is_err() {
                OLD_OPTIMIZED_MIR.store(providers.optimized_mir as usize, SeqCst);
                providers.optimized_mir = Self::custom_optimized_mir;
            } else {
                trace!("Not instrumenting crate {:?}", session.opts.crate_name);
            }
        });
    }
}

#[cfg(unix)]
static ENTRY_FN: OnceCell<Option<DefId>> = OnceCell::new();

pub struct DynamicRTSCallbacks {
    path: PathBuf,
    context: OnceCell<RTSContext>,
}

impl DynamicRTSCallbacks {
    pub fn new(target_dir: PathBuf) -> Self {
        Self {
            path: target_dir,
            context: OnceCell::new(),
        }
    }
}

// impl InstrumentingCallback for DynamicRTSCallbacks {
//     fn modify_body<'tcx>(tcx: TyCtxt<'tcx>, body: &mut Body<'tcx>) {
//         let def_id = body.source.instance.def_id();
//         let outer = def_id_name(tcx, def_id, false, true);

//         trace!("Visiting {}", outer);

//         let mut cache_ret = None;

//         let attrs = &tcx.hir_crate(()).owners[tcx
//             .local_def_id_to_hir_id(def_id.expect_local())
//             .owner
//             .def_id]
//             .as_owner()
//             .map_or(AttributeMap::EMPTY, |o| &o.attrs)
//             .map;

//         for (_, list) in attrs.iter() {
//             for attr in *list {
//                 if attr.name_or_empty().to_ident_string() == TEST_MARKER {
//                     let def_path = def_id_name(tcx, def_id, false, false);
//                     let def_path_test = &def_path[0..def_path.len() - 13];

//                     // IMPORTANT: The order in which insert_post, insert_pre are called is critical here
//                     // 1. insert_post 2. insert_pre

//                     body.insert_post_test(tcx, def_path_test, &mut cache_ret, &mut None, false);
//                     body.insert_pre_test(tcx, &mut cache_ret);
//                     return;
//                 }
//             }
//         }

//         #[cfg(unix)]
//         if let Some(entry_def) = ENTRY_FN.get_or_init(|| tcx.entry_fn(()).map(|(def_id, _)| def_id))
//         {
//             if def_id == *entry_def {
//                 // IMPORTANT: The order in which insert_post, trace, insert_pre are called is critical here
//                 // 1. insert_post, 2. trace, 3. insert_pre

//                 body.insert_post_main(tcx, &mut cache_ret, &mut None);
//             }
//         }

//         body.insert_trace(tcx, &outer, &mut cache_ret);

//         #[cfg(unix)]
//         body.check_calls_to_exit(tcx, &mut cache_ret);

//         #[cfg(unix)]
//         if let Some(entry_def) = ENTRY_FN.get().unwrap() {
//             if def_id == *entry_def {
//                 body.insert_pre_main(tcx, &mut cache_ret);
//             }
//         }
//     }
// }

impl<'tcx> AnalysisCallback<'tcx> for DynamicRTSCallbacks {}

impl ChecksumsCallback for DynamicRTSCallbacks {
    fn path(&self) -> &std::path::Path {
        &self.path
    }

    fn context(&self) -> &RTSContext {
        self.context.get().expect("Context not yet initilaized")
    }

    fn context_mut(&mut self) -> &mut RTSContext {
        self.context.get_mut().expect("Context not yet initilaized")
    }
}

impl Drop for DynamicRTSCallbacks {
    fn drop(&mut self) {
        if self.context.get().is_some() {
            let old_checksums = self.import_checksums(ChecksumKind::Checksum, false);
            let old_checksums_vtbl = self.import_checksums(ChecksumKind::VtblChecksum, false);
            let old_checksums_const = self.import_checksums(ChecksumKind::ConstChecksum, false);

            let context = &mut self.context.get_mut().unwrap();

            context.old_checksums.get_or_init(|| old_checksums);
            context
                .old_checksums_vtbl
                .get_or_init(|| old_checksums_vtbl);
            context
                .old_checksums_const
                .get_or_init(|| old_checksums_const);

            let new_checksums_vtbl = &*NEW_CHECKSUMS_VTBL.get().unwrap().lock().unwrap();
            context
                .new_checksums_vtbl
                .get_or_init(|| new_checksums_vtbl.clone());

            self.export_changes(CacheKind::Dynamic);

            let RTSContext {
                new_checksums,
                new_checksums_vtbl,
                new_checksums_const,
                ..
            } = self.context();

            self.export_checksums(ChecksumKind::Checksum, new_checksums.get().unwrap(), false);
            self.export_checksums(
                ChecksumKind::VtblChecksum,
                new_checksums_vtbl.get().unwrap(),
                false,
            );
            self.export_checksums(
                ChecksumKind::ConstChecksum,
                new_checksums_const.get().unwrap(),
                false,
            );
        } else {
            debug!("Aborting without exporting changes");
        }
    }
}

impl Callbacks for DynamicRTSCallbacks {
    fn config(&mut self, config: &mut Config) {
        if std::env::var(ENV_SKIP_INSTRUMENTATION).is_err() {
            let file_loader = Box::new(TestRunnerFileLoaderProxy {
                delegate: InstrumentationFileLoaderProxy {
                    delegate: RealFileLoader,
                },
            })
                as Box<dyn FileLoader + std::marker::Send + std::marker::Sync>;
            config.file_loader = Some(file_loader);
        }

        // We need to replace this in any case, since we also want to instrument rlib crates
        // Further, the only possibility to intercept vtable entries, which I found, is in their local crate
        config.override_queries = Some(|session, providers| {
            debug!("Modifying providers");

            if std::env::var(ENV_SKIP_INSTRUMENTATION).is_err() {
                OLD_OPTIMIZED_MIR.store(providers.optimized_mir as usize, SeqCst);
                providers.optimized_mir = InstrumentingRTSCallbacks::custom_optimized_mir;
            } else {
                trace!("Not instrumenting crate {:?}", session.opts.crate_name);
            }

            if std::env::var(ENV_SKIP_ANALYSIS).is_err() {
                OLD_VTABLE_ENTRIES.store(providers.vtable_entries as usize, SeqCst);
                providers.vtable_entries = |tcx, binder| {
                    <Self as AnalysisCallback>::custom_vtable_entries(tcx, binder, "")
                };
            } else {
                trace!("Not analyzing crate {:?}", session.opts.crate_name);
            }
        });
    }

    fn after_crate_root_parsing<'tcx>(
        &mut self,
        _compiler: &interface::Compiler,
        queries: &'tcx Queries<'tcx>,
    ) -> Compilation {
        queries.global_ctxt().unwrap().enter(|tcx| {
            {
                let context = self.init_analysis(tcx);
                self.context.get_or_init(|| context);

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
        });

        Compilation::Continue
    }

    fn after_analysis<'tcx>(
        &mut self,
        _compiler: &interface::Compiler,
        queries: &'tcx Queries<'tcx>,
    ) -> Compilation {
        if std::env::var(ENV_SKIP_ANALYSIS).is_err() {
            queries.global_ctxt().unwrap().enter(|tcx| {
                let context = self.init_analysis(tcx);
                self.context.get_or_init(|| context);

                self.run_analysis_shared(tcx);
            });
        }

        Compilation::Continue
    }
}

pub trait InstrumentingCallback {
    fn modify_body<'tcx>(tcx: TyCtxt<'tcx>, body: &mut Body<'tcx>);

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

        //##############################################################
        // 1. Here the MIR is modified to trace this function at runtime

        Self::modify_body(tcx, result);

        result
    }
}
