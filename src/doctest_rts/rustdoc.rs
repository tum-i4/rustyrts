#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(clippy::while_let_on_iterator)]
#![allow(clippy::redundant_closure)]
#![allow(clippy::if_same_then_else)]

use std::{borrow::Cow, env, iter::Peekable, path::PathBuf, str::CharIndices, sync::Arc};

use pulldown_cmark::{CodeBlockKind, Event, Parser, Tag};
use rustc_ast::ast;

use rustc_data_structures::fx::FxHashMap;
use rustc_error_messages::DiagnosticMessage;
use rustc_errors::DiagnosticBuilder;
use rustc_hir::intravisit::{self};

use rustc_middle::{
    hir::{map::Map, nested_filter},
    ty::TyCtxt,
};
use rustc_resolve::rustdoc::{
    add_doc_fragment, attrs_to_doc_fragments, span_of_fragments, DocFragment,
};

use rustc_session::Session;
use rustc_span::{
    def_id::{DefId, LocalDefId},
    edition::Edition,
    source_map::SourceMap,
    BytePos, FileName, Pos, Span, DUMMY_SP,
};

impl<'a, 'hir, 'tcx> HirCollector<'a, 'hir, 'tcx> {
    pub(crate) fn new(
        sess: &'a Session,
        collector: &'a mut Collector,
        map: Map<'hir>,
        codes: ErrorCodes,
        tcx: TyCtxt<'tcx>,
    ) -> Self {
        Self {
            sess,
            collector,
            map,
            codes,
            tcx,
        }
    }
}

//#####################################################################################################################
// Source: https://doc.rust-lang.org/1.77.0/nightly-rustc/src/rustdoc/doctest.rs.html#1197

pub(crate) struct HirCollector<'a, 'hir, 'tcx> {
    sess: &'a Session,
    collector: &'a mut Collector,
    map: Map<'hir>,
    codes: ErrorCodes,
    tcx: TyCtxt<'tcx>,
}

impl<'a, 'hir, 'tcx> HirCollector<'a, 'hir, 'tcx> {
    pub(crate) fn visit_testable<F: FnOnce(&mut Self)>(
        // CHANGED: to crate public
        &mut self,
        name: String,
        def_id: LocalDefId,
        sp: Span,
        nested: F,
    ) {
        let ast_attrs = self
            .tcx
            .hir()
            .attrs(self.tcx.local_def_id_to_hir_id(def_id));
        // if let Some(ref cfg) = ast_attrs.cfg(self.tcx, &FxHashSet::default()) {
        //     if !cfg.matches(&self.sess.parse_sess, Some(self.tcx.features())) {
        //         return;
        //     }
        // }

        let has_name = !name.is_empty();
        if has_name {
            self.collector.names.push(name);
        }

        // The collapse-docs pass won't combine sugared/raw doc attributes, or included files with
        // anything else, this will combine them for us.
        let attrs = Attributes::from_ast(ast_attrs);
        if let Some(doc) = attrs.opt_doc_value() {
            // Use the outermost invocation, so that doctest names come from where the docs were written.
            let span = ast_attrs
                .iter()
                .find(|attr| attr.doc_str().is_some())
                .map(|attr| {
                    attr.span
                        .ctxt()
                        .outer_expn()
                        .expansion_cause()
                        .unwrap_or(attr.span)
                })
                .unwrap_or(DUMMY_SP);
            self.collector.set_position(span);
            find_testable_code(
                &doc,
                self.collector,
                self.codes,
                self.collector.enable_per_target_ignores,
                Some(&ExtraInfo::new(
                    self.tcx,
                    def_id.to_def_id(),
                    span_of_fragments(&attrs.doc_strings).unwrap_or(sp),
                )),
                self.tcx.features().custom_code_classes_in_docs,
            );
        }

        nested(self);

        if has_name {
            self.collector.names.pop();
        }
    }
}

impl<'a, 'hir, 'tcx> intravisit::Visitor<'hir> for HirCollector<'a, 'hir, 'tcx> {
    type NestedFilter = nested_filter::All;

    fn nested_visit_map(&mut self) -> Self::Map {
        self.map
    }

    fn visit_item(&mut self, item: &'hir rustc_hir::Item<'_>) {
        let name = match &item.kind {
            rustc_hir::ItemKind::Impl(impl_) => {
                rustc_hir_pretty::id_to_string(&self.map, impl_.self_ty.hir_id)
            }
            _ => item.ident.to_string(),
        };

        self.visit_testable(name, item.owner_id.def_id, item.span, |this| {
            intravisit::walk_item(this, item);
        });
    }

    fn visit_trait_item(&mut self, item: &'hir rustc_hir::TraitItem<'_>) {
        self.visit_testable(
            item.ident.to_string(),
            item.owner_id.def_id,
            item.span,
            |this| {
                intravisit::walk_trait_item(this, item);
            },
        );
    }

    fn visit_impl_item(&mut self, item: &'hir rustc_hir::ImplItem<'_>) {
        self.visit_testable(
            item.ident.to_string(),
            item.owner_id.def_id,
            item.span,
            |this| {
                intravisit::walk_impl_item(this, item);
            },
        );
    }

    fn visit_foreign_item(&mut self, item: &'hir rustc_hir::ForeignItem<'_>) {
        self.visit_testable(
            item.ident.to_string(),
            item.owner_id.def_id,
            item.span,
            |this| {
                intravisit::walk_foreign_item(this, item);
            },
        );
    }

    fn visit_variant(&mut self, v: &'hir rustc_hir::Variant<'_>) {
        self.visit_testable(v.ident.to_string(), v.def_id, v.span, |this| {
            intravisit::walk_variant(this, v);
        });
    }

    fn visit_field_def(&mut self, f: &'hir rustc_hir::FieldDef<'_>) {
        self.visit_testable(f.ident.to_string(), f.def_id, f.span, |this| {
            intravisit::walk_field_def(this, f);
        });
    }
}

//#####################################################################################################################
// Source: https://doc.rust-lang.org/1.77.0/nightly-rustc/src/rustdoc/clean/types.rs.html#1138

/// The attributes on an [`Item`], including attributes like `#[derive(...)]` and `#[inline]`,
/// as well as doc comments.
#[derive(Clone, Debug, Default)]
pub(crate) struct Attributes {
    pub(crate) doc_strings: Vec<DocFragment>,
    pub(crate) other_attrs: ast::AttrVec,
}

impl Attributes {
    pub(crate) fn from_ast(attrs: &[ast::Attribute]) -> Attributes {
        Attributes::from_ast_iter(attrs.iter().map(|attr| (attr, None)), false)
    }

    pub(crate) fn from_ast_iter<'a>(
        attrs: impl Iterator<Item = (&'a ast::Attribute, Option<DefId>)>,
        doc_only: bool,
    ) -> Attributes {
        let (doc_strings, other_attrs) = attrs_to_doc_fragments(attrs, doc_only);
        Attributes {
            doc_strings,
            other_attrs,
        }
    }

    /// Combine all doc strings into a single value handling indentation and newlines as needed.
    /// Returns `None` is there's no documentation at all, and `Some("")` if there is some
    /// documentation but it is empty (e.g. `#[doc = ""]`).
    pub(crate) fn opt_doc_value(&self) -> Option<String> {
        (!self.doc_strings.is_empty()).then(|| {
            let mut res = String::new();
            for frag in &self.doc_strings {
                add_doc_fragment(&mut res, frag);
            }
            res.pop();
            res
        })
    }
}

//#####################################################################################################################
// Source: https://doc.rust-lang.org/1.77.0/nightly-rustc/src/rustdoc/html/markdown.rs.html#858-870

#[derive(Eq, PartialEq, Clone, Debug)]
pub(crate) struct LangString {
    pub(crate) original: String,
    pub(crate) should_panic: bool,
    pub(crate) no_run: bool,
    pub(crate) ignore: Ignore,
    pub(crate) rust: bool,
    pub(crate) test_harness: bool,
    pub(crate) compile_fail: bool,
    pub(crate) error_codes: Vec<String>,
    pub(crate) edition: Option<Edition>,
    pub(crate) added_classes: Vec<String>,
    pub(crate) unknown: Vec<String>,
}

//#####################################################################################################################
// Source: https://doc.rust-lang.org/1.77.0/nightly-rustc/src/rustdoc/html/markdown.rs.html#1224-1394

impl Default for LangString {
    fn default() -> Self {
        Self {
            original: String::new(),
            should_panic: false,
            no_run: false,
            ignore: Ignore::None,
            rust: true,
            test_harness: false,
            compile_fail: false,
            error_codes: Vec::new(),
            edition: None,
            added_classes: Vec::new(),
            unknown: Vec::new(),
        }
    }
}

impl LangString {
    fn parse_without_check(
        string: &str,
        allow_error_code_check: ErrorCodes,
        enable_per_target_ignores: bool,
        custom_code_classes_in_docs: bool,
    ) -> Self {
        Self::parse(
            string,
            allow_error_code_check,
            enable_per_target_ignores,
            None,
            custom_code_classes_in_docs,
        )
    }

    fn parse(
        string: &str,
        allow_error_code_check: ErrorCodes,
        enable_per_target_ignores: bool,
        extra: Option<&ExtraInfo<'_>>,
        custom_code_classes_in_docs: bool,
    ) -> Self {
        let allow_error_code_check = allow_error_code_check.as_bool();
        let mut seen_rust_tags = false;
        let mut seen_other_tags = false;
        let mut seen_custom_tag = false;
        let mut data = LangString::default();
        let mut ignores = vec![];

        data.original = string.to_owned();

        let mut call = |tokens: &mut dyn Iterator<Item = LangStringToken<'_>>| {
            for token in tokens {
                match token {
                    LangStringToken::LangToken("should_panic") => {
                        data.should_panic = true;
                        seen_rust_tags = !seen_other_tags;
                    }
                    LangStringToken::LangToken("no_run") => {
                        data.no_run = true;
                        seen_rust_tags = !seen_other_tags;
                    }
                    LangStringToken::LangToken("ignore") => {
                        data.ignore = Ignore::All;
                        seen_rust_tags = !seen_other_tags;
                    }
                    LangStringToken::LangToken(x) if x.starts_with("ignore-") => {
                        if enable_per_target_ignores {
                            ignores.push(x.trim_start_matches("ignore-").to_owned());
                            seen_rust_tags = !seen_other_tags;
                        }
                    }
                    LangStringToken::LangToken("rust") => {
                        data.rust = true;
                        seen_rust_tags = true;
                    }
                    LangStringToken::LangToken("custom") => {
                        if custom_code_classes_in_docs {
                            seen_custom_tag = true;
                        } else {
                            seen_other_tags = true;
                        }
                    }
                    LangStringToken::LangToken("test_harness") => {
                        data.test_harness = true;
                        seen_rust_tags = !seen_other_tags || seen_rust_tags;
                    }
                    LangStringToken::LangToken("compile_fail") => {
                        data.compile_fail = true;
                        seen_rust_tags = !seen_other_tags || seen_rust_tags;
                        data.no_run = true;
                    }
                    LangStringToken::LangToken(x) if x.starts_with("edition") => {
                        data.edition = x[7..].parse::<Edition>().ok();
                    }
                    LangStringToken::LangToken(x)
                        if x.starts_with("rust") && x[4..].parse::<Edition>().is_ok() =>
                    {
                        if let Some(extra) = extra {
                            extra.error_invalid_codeblock_attr_with_help(
                                format!("unknown attribute `{x}`"),
                                |lint| {
                                    lint.help(format!(
                                        "there is an attribute with a similar name: `edition{}`",
                                        &x[4..],
                                    ));
                                },
                            );
                        }
                    }
                    LangStringToken::LangToken(x)
                        if allow_error_code_check && x.starts_with('E') && x.len() == 5 =>
                    {
                        if x[1..].parse::<u32>().is_ok() {
                            data.error_codes.push(x.to_owned());
                            seen_rust_tags = !seen_other_tags || seen_rust_tags;
                        } else {
                            seen_other_tags = true;
                        }
                    }
                    LangStringToken::LangToken(x) if extra.is_some() => {
                        let s = x.to_lowercase();
                        if let Some((flag, help)) = if s == "compile-fail"
                            || s == "compile_fail"
                            || s == "compilefail"
                        {
                            Some((
                                "compile_fail",
                                "the code block will either not be tested if not marked as a rust one \
                                 or won't fail if it compiles successfully",
                            ))
                        } else if s == "should-panic" || s == "should_panic" || s == "shouldpanic" {
                            Some((
                                "should_panic",
                                "the code block will either not be tested if not marked as a rust one \
                                 or won't fail if it doesn't panic when running",
                            ))
                        } else if s == "no-run" || s == "no_run" || s == "norun" {
                            Some((
                                "no_run",
                                "the code block will either not be tested if not marked as a rust one \
                                 or will be run (which you might not want)",
                            ))
                        } else if s == "test-harness" || s == "test_harness" || s == "testharness" {
                            Some((
                                "test_harness",
                                "the code block will either not be tested if not marked as a rust one \
                                 or the code will be wrapped inside a main function",
                            ))
                        } else {
                            None
                        } {
                            if let Some(extra) = extra {
                                extra.error_invalid_codeblock_attr_with_help(
                                    format!("unknown attribute `{x}`"),
                                    |lint| {
                                        lint.help(format!(
                                            "there is an attribute with a similar name: `{flag}`"
                                        ))
                                        .help(help);
                                    },
                                );
                            }
                        }
                        seen_other_tags = true;
                        data.unknown.push(x.to_owned());
                    }
                    LangStringToken::LangToken(x) => {
                        seen_other_tags = true;
                        data.unknown.push(x.to_owned());
                    }
                    LangStringToken::KeyValueAttribute(key, value) => {
                        if custom_code_classes_in_docs {
                            if key == "class" {
                                data.added_classes.push(value.to_owned());
                            } else if let Some(extra) = extra {
                                extra.error_invalid_codeblock_attr(format!(
                                    "unsupported attribute `{key}`"
                                ));
                            }
                        } else {
                            seen_other_tags = true;
                        }
                    }
                    LangStringToken::ClassAttribute(class) => {
                        data.added_classes.push(class.to_owned());
                    }
                }
            }
        };

        if custom_code_classes_in_docs {
            call(&mut TagIterator::new(string, extra))
        } else {
            call(&mut tokens(string))
        }

        // ignore-foo overrides ignore
        if !ignores.is_empty() {
            data.ignore = Ignore::Some(ignores);
        }

        data.rust &= !seen_custom_tag && (!seen_other_tags || seen_rust_tags);

        data
    }
}

//#####################################################################################################################
// Source: https://doc.rust-lang.org/1.77.0/nightly-rustc/src/rustdoc/html/markdown.rs.html#872-877

#[derive(Eq, PartialEq, Clone, Debug)]
pub(crate) enum Ignore {
    All,
    None,
    Some(Vec<String>),
}

//#####################################################################################################################
// Source: https://doc.rust-lang.org/1.77.0/nightly-rustc/src/rustdoc/doctest.rs.html#874

pub(crate) trait Tester {
    fn add_test(&mut self, test: String, config: LangString, line: usize);
    fn get_line(&self) -> usize {
        0
    }
    fn register_header(&mut self, _name: &str, _level: u32) {}
}

//#####################################################################################################################
// Source: https://doc.rust-lang.org/1.77.0/nightly-rustc/src/rustdoc/doctest.rs.html#882-1195

pub(crate) struct Collector {
    pub(crate) tests: Vec<(String, (String, GlobalTestOptions, LangString))>, // CHANGED: ...to fit what we need

    // The name of the test displayed to the user, separated by `::`.
    //
    // In tests from Rust source, this is the path to the item
    // e.g., `["std", "vec", "Vec", "push"]`.
    //
    // In tests from a markdown file, this is the titles of all headers (h1~h6)
    // of the sections that contain the code block, e.g., if the markdown file is
    // written as:
    //
    // ``````markdown
    // # Title
    //
    // ## Subtitle
    //
    // ```rust
    // assert!(true);
    // ```
    // ``````
    //
    // the `names` vector of that test will be `["Title", "Subtitle"]`.
    names: Vec<String>,

    // rustdoc_options: RustdocOptions,
    use_headers: bool,
    enable_per_target_ignores: bool,
    crate_name: String,
    opts: GlobalTestOptions,
    position: Span,
    source_map: Option<Arc<SourceMap>>,
    filename: Option<PathBuf>,
    visited_tests: FxHashMap<(String, usize), usize>,
    // unused_extern_reports: Arc<Mutex<Vec<UnusedExterns>>>,
    // compiling_test_count: AtomicUsize,
}

impl Collector {
    pub(crate) fn new(
        crate_name: String,
        // rustdoc_options: RustdocOptions,
        use_headers: bool,
        opts: GlobalTestOptions,
        source_map: Option<Arc<SourceMap>>,
        filename: Option<PathBuf>,
        enable_per_target_ignores: bool,
    ) -> Collector {
        Collector {
            tests: Vec::new(),
            names: Vec::new(),
            // rustdoc_options,
            use_headers,
            enable_per_target_ignores,
            crate_name,
            opts,
            position: DUMMY_SP,
            source_map,
            filename,
            visited_tests: FxHashMap::default(),
            // unused_extern_reports: Default::default(),
            // compiling_test_count: AtomicUsize::new(0),
        }
    }

    fn generate_name(&self, line: usize, filename: &FileName) -> String {
        let mut item_path = self.names.join("::");
        item_path.retain(|c| c != ' ');
        if !item_path.is_empty() {
            item_path.push(' ');
        }
        format!("{} - {item_path}(line {line})", filename.prefer_local())
    }

    pub(crate) fn set_position(&mut self, position: Span) {
        self.position = position;
    }

    fn get_filename(&self) -> FileName {
        if let Some(ref source_map) = self.source_map {
            let filename = source_map.span_to_filename(self.position);
            if let FileName::Real(ref filename) = filename
                && let Ok(cur_dir) = env::current_dir()
                && let Some(local_path) = filename.local_path()
                && let Ok(path) = local_path.strip_prefix(&cur_dir)
            {
                return path.to_owned().into();
            }
            filename
        } else if let Some(ref filename) = self.filename {
            filename.clone().into()
        } else {
            FileName::Custom("input".to_owned())
        }
    }
}

impl Tester for Collector {
    fn add_test(&mut self, test: String, config: LangString, line: usize) {
        let filename = self.get_filename();
        let name = self.generate_name(line, &filename);
        let crate_name = self.crate_name.clone();
        // let opts = self.opts.clone();
        // let edition = config.edition.unwrap_or(self.rustdoc_options.edition);
        // let rustdoc_options = self.rustdoc_options.clone();
        // let runtool = self.rustdoc_options.runtool.clone();
        // let runtool_args = self.rustdoc_options.runtool_args.clone();
        // let target = self.rustdoc_options.target.clone();
        // let target_str = target.to_string();
        // let unused_externs = self.unused_extern_reports.clone();
        // let no_run = config.no_run || rustdoc_options.no_run;
        // if !config.compile_fail {
        //     self.compiling_test_count.fetch_add(1, Ordering::SeqCst);
        // }

        let path = match &filename {
            FileName::Real(path) => {
                if let Some(local_path) = path.local_path() {
                    local_path.to_path_buf()
                } else {
                    // Somehow we got the filename from the metadata of another crate, should never happen
                    unreachable!("doctest from a different crate");
                }
            }
            _ => PathBuf::from(r"doctest.rs"),
        };

        // For example `module/file.rs` would become `module_file_rs`
        let file = filename
            .prefer_local()
            .to_string_lossy()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .collect::<String>();
        let test_id = format!(
            "{file}_{line}_{number}",
            file = file,
            line = line,
            number = {
                // Increases the current test number, if this file already
                // exists or it creates a new entry with a test number of 0.
                self.visited_tests
                    .entry((file.clone(), line))
                    .and_modify(|v| *v += 1)
                    .or_insert(0)
            },
        );

        // CHANGED: only extract name, source code and lang string
        self.tests.push((name, (test, self.opts.clone(), config)));

        // CHANGED:
        // let outdir = if let Some(mut path) = rustdoc_options.persist_doctests.clone() {
        //     path.push(&test_id);

        //     if let Err(err) = std::fs::create_dir_all(&path) {
        //         eprintln!("Couldn't create directory for doctest executables: {err}");
        //         panic::resume_unwind(Box::new(()));
        //     }

        //     DirState::Perm(path)
        // } else {
        //     DirState::Temp(
        //         TempFileBuilder::new()
        //             .prefix("rustdoctest")
        //             .tempdir()
        //             .expect("rustdoc needs a tempdir"),
        //     )
        // };

        // debug!("creating test {name}: {test}");
        // self.tests.push(test::TestDescAndFn {
        //     desc: test::TestDesc {
        //         name: test::DynTestName(name),
        //         ignore: match config.ignore {
        //             Ignore::All => true,
        //             Ignore::None => false,
        //             Ignore::Some(ref ignores) => ignores.iter().any(|s| target_str.contains(s)),
        //         },
        //         ignore_message: None,
        //         source_file: "",
        //         start_line: 0,
        //         start_col: 0,
        //         end_line: 0,
        //         end_col: 0,
        //         // compiler failures are test failures
        //         should_panic: test::ShouldPanic::No,
        //         compile_fail: config.compile_fail,
        //         no_run,
        //         test_type: test::TestType::DocTest,
        //     },
        //     testfn: test::DynTestFn(Box::new(move || {
        //         let report_unused_externs = |uext| {
        //             unused_externs.lock().unwrap().push(uext);
        //         };
        //         let res = run_test(
        //             &test,
        //             &crate_name,
        //             line,
        //             rustdoc_options,
        //             config,
        //             no_run,
        //             runtool,
        //             runtool_args,
        //             target,
        //             &opts,
        //             edition,
        //             outdir,
        //             path,
        //             &test_id,
        //             report_unused_externs,
        //         );

        //         if let Err(err) = res {
        //             match err {
        //                 TestFailure::CompileError => {
        //                     eprint!("Couldn't compile the test.");
        //                 }
        //                 TestFailure::UnexpectedCompilePass => {
        //                     eprint!("Test compiled successfully, but it's marked `compile_fail`.");
        //                 }
        //                 TestFailure::UnexpectedRunPass => {
        //                     eprint!("Test executable succeeded, but it's marked `should_panic`.");
        //                 }
        //                 TestFailure::MissingErrorCodes(codes) => {
        //                     eprint!("Some expected error codes were not found: {codes:?}");
        //                 }
        //                 TestFailure::ExecutionError(err) => {
        //                     eprint!("Couldn't run the test: {err}");
        //                     if err.kind() == io::ErrorKind::PermissionDenied {
        //                         eprint!(" - maybe your tempdir is mounted with noexec?");
        //                     }
        //                 }
        //                 TestFailure::ExecutionFailure(out) => {
        //                     eprintln!("Test executable failed ({reason}).", reason = out.status);

        //                     // FIXME(#12309): An unfortunate side-effect of capturing the test
        //                     // executable's output is that the relative ordering between the test's
        //                     // stdout and stderr is lost. However, this is better than the
        //                     // alternative: if the test executable inherited the parent's I/O
        //                     // handles the output wouldn't be captured at all, even on success.
        //                     //
        //                     // The ordering could be preserved if the test process' stderr was
        //                     // redirected to stdout, but that functionality does not exist in the
        //                     // standard library, so it may not be portable enough.
        //                     let stdout = str::from_utf8(&out.stdout).unwrap_or_default();
        //                     let stderr = str::from_utf8(&out.stderr).unwrap_or_default();

        //                     if !stdout.is_empty() || !stderr.is_empty() {
        //                         eprintln!();

        //                         if !stdout.is_empty() {
        //                             eprintln!("stdout:\n{stdout}");
        //                         }

        //                         if !stderr.is_empty() {
        //                             eprintln!("stderr:\n{stderr}");
        //                         }
        //                     }
        //                 }
        //             }

        //             panic::resume_unwind(Box::new(()));
        //         }
        //         Ok(())
        //     })),
        // });
    }

    fn get_line(&self) -> usize {
        if let Some(ref source_map) = self.source_map {
            let line = self.position.lo().to_usize();
            let line = source_map.lookup_char_pos(BytePos(line as u32)).line;
            if line > 0 {
                line - 1
            } else {
                line
            }
        } else {
            0
        }
    }

    fn register_header(&mut self, name: &str, level: u32) {
        if self.use_headers {
            // We use these headings as test names, so it's good if
            // they're valid identifiers.
            let name = name
                .chars()
                .enumerate()
                .map(|(i, c)| {
                    if (i == 0 && rustc_lexer::is_id_start(c))
                        || (i != 0 && rustc_lexer::is_id_continue(c))
                    {
                        c
                    } else {
                        '_'
                    }
                })
                .collect::<String>();

            // Here we try to efficiently assemble the header titles into the
            // test name in the form of `h1::h2::h3::h4::h5::h6`.
            //
            // Suppose that originally `self.names` contains `[h1, h2, h3]`...
            let level = level as usize;
            if level <= self.names.len() {
                // ... Consider `level == 2`. All headers in the lower levels
                // are irrelevant in this new level. So we should reset
                // `self.names` to contain headers until <h2>, and replace that
                // slot with the new name: `[h1, name]`.
                self.names.truncate(level);
                self.names[level - 1] = name;
            } else {
                // ... On the other hand, consider `level == 5`. This means we
                // need to extend `self.names` to contain five headers. We fill
                // in the missing level (<h4>) with `_`. Thus `self.names` will
                // become `[h1, h2, h3, "_", name]`.
                if level - 1 > self.names.len() {
                    self.names.resize(level - 1, "_".to_owned());
                }
                self.names.push(name);
            }
        }
    }
}

//#####################################################################################################################
// Source: https://doc.rust-lang.org/1.77.0/nightly-rustc/src/rustdoc/html/markdown.rs.html#727-815

pub(crate) fn find_testable_code<T: Tester>(
    doc: &str,
    tests: &mut T,
    error_codes: ErrorCodes,
    enable_per_target_ignores: bool,
    extra_info: Option<&ExtraInfo<'_>>,
    custom_code_classes_in_docs: bool,
) {
    find_codes(
        doc,
        tests,
        error_codes,
        enable_per_target_ignores,
        extra_info,
        false,
        custom_code_classes_in_docs,
    )
}

pub(crate) fn find_codes<T: Tester>(
    doc: &str,
    tests: &mut T,
    error_codes: ErrorCodes,
    enable_per_target_ignores: bool,
    extra_info: Option<&ExtraInfo<'_>>,
    include_non_rust: bool,
    custom_code_classes_in_docs: bool,
) {
    let mut parser = Parser::new(doc).into_offset_iter();
    let mut prev_offset = 0;
    let mut nb_lines = 0;
    let mut register_header = None;
    while let Some((event, offset)) = parser.next() {
        match event {
            Event::Start(Tag::CodeBlock(kind)) => {
                let block_info = match kind {
                    CodeBlockKind::Fenced(ref lang) => {
                        if lang.is_empty() {
                            Default::default()
                        } else {
                            LangString::parse(
                                lang,
                                error_codes,
                                enable_per_target_ignores,
                                extra_info,
                                custom_code_classes_in_docs,
                            )
                        }
                    }
                    CodeBlockKind::Indented => Default::default(),
                };
                if !include_non_rust && !block_info.rust {
                    continue;
                }

                let mut test_s = String::new();

                while let Some((Event::Text(s), _)) = parser.next() {
                    test_s.push_str(&s);
                }
                let text = test_s
                    .lines()
                    .map(|l| map_line(l).for_code())
                    .collect::<Vec<Cow<'_, str>>>()
                    .join("\n");

                nb_lines += doc[prev_offset..offset.start].lines().count();
                // If there are characters between the preceding line ending and
                // this code block, `str::lines` will return an additional line,
                // which we subtract here.
                if nb_lines != 0 && !&doc[prev_offset..offset.start].ends_with('\n') {
                    nb_lines -= 1;
                }
                let line = tests.get_line() + nb_lines + 1;
                tests.add_test(text, block_info, line);
                prev_offset = offset.start;
            }
            Event::Start(Tag::Heading {
                level,
                id: _,
                classes,
                attrs: _,
            }) => {
                register_header = Some(level as u32);
            }
            Event::Text(ref s) if register_header.is_some() => {
                let level = register_header.unwrap();
                tests.register_header(s, level);
                register_header = None;
            }
            _ => {}
        }
    }
}

//#####################################################################################################################
// Source: https://doc.rust-lang.org/1.77.0/nightly-rustc/src/rustdoc/html/markdown.rs.html#817-855

pub(crate) struct ExtraInfo<'tcx> {
    def_id: DefId,
    sp: Span,
    tcx: TyCtxt<'tcx>,
}

impl<'tcx> ExtraInfo<'tcx> {
    pub(crate) fn new(tcx: TyCtxt<'tcx>, def_id: DefId, sp: Span) -> ExtraInfo<'tcx> {
        ExtraInfo { def_id, sp, tcx }
    }

    fn error_invalid_codeblock_attr(&self, msg: impl Into<DiagnosticMessage>) {
        // if let Some(def_id) = self.def_id.as_local() {
        //     self.tcx.node_span_lint(
        //         crate::lint::INVALID_CODEBLOCK_ATTRIBUTES,
        //         self.tcx.local_def_id_to_hir_id(def_id),
        //         self.sp,
        //         msg,
        //         |_| {},
        //     );
        // }
    }

    fn error_invalid_codeblock_attr_with_help(
        &self,
        msg: impl Into<DiagnosticMessage>,
        f: impl for<'a, 'b> FnOnce(&'b mut DiagnosticBuilder<'a, ()>),
    ) {
        // if let Some(def_id) = self.def_id.as_local() {
        //     self.tcx.node_span_lint(
        //         crate::lint::INVALID_CODEBLOCK_ATTRIBUTES,
        //         self.tcx.local_def_id_to_hir_id(def_id),
        //         self.sp,
        //         msg,
        //         f,
        //     );
        // }
    }
}

//#####################################################################################################################
// Source: https://doc.rust-lang.org/1.77.0/nightly-rustc/src/rustdoc/html/markdown.rs.html#118-138

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum ErrorCodes {
    Yes,
    No,
}

impl ErrorCodes {
    pub(crate) fn from(b: bool) -> Self {
        match b {
            true => ErrorCodes::Yes,
            false => ErrorCodes::No,
        }
    }

    pub(crate) fn as_bool(self) -> bool {
        match self {
            ErrorCodes::Yes => true,
            ErrorCodes::No => false,
        }
    }
}

//#####################################################################################################################
// Source: https://doc.rust-lang.org/1.77.0/nightly-rustc/src/rustdoc/html/markdown.rs.html#879-1188

/// This is the parser for fenced codeblocks attributes. It implements the following eBNF:
///
/// ```eBNF
/// lang-string = *(token-list / delimited-attribute-list / comment)
///
/// bareword = LEADINGCHAR *(CHAR)
/// bareword-without-leading-char = CHAR *(CHAR)
/// quoted-string = QUOTE *(NONQUOTE) QUOTE
/// token = bareword / quoted-string
/// token-without-leading-char = bareword-without-leading-char / quoted-string
/// sep = COMMA/WS *(COMMA/WS)
/// attribute = (DOT token)/(token EQUAL token-without-leading-char)
/// attribute-list = [sep] attribute *(sep attribute) [sep]
/// delimited-attribute-list = OPEN-CURLY-BRACKET attribute-list CLOSE-CURLY-BRACKET
/// token-list = [sep] token *(sep token) [sep]
/// comment = OPEN_PAREN *(all characters) CLOSE_PAREN
///
/// OPEN_PAREN = "("
/// CLOSE_PARENT = ")"
/// OPEN-CURLY-BRACKET = "{"
/// CLOSE-CURLY-BRACKET = "}"
/// LEADINGCHAR = ALPHA | DIGIT | "_" | "-" | ":"
/// ; All ASCII punctuation except comma, quote, equals, backslash, grave (backquote) and braces.
/// ; Comma is used to separate language tokens, so it can't be used in one.
/// ; Quote is used to allow otherwise-disallowed characters in language tokens.
/// ; Equals is used to make key=value pairs in attribute blocks.
/// ; Backslash and grave are special Markdown characters.
/// ; Braces are used to start an attribute block.
/// CHAR = ALPHA | DIGIT | "_" | "-" | ":" | "." | "!" | "#" | "$" | "%" | "&" | "*" | "+" | "/" |
///        ";" | "<" | ">" | "?" | "@" | "^" | "|" | "~"
/// NONQUOTE = %x09 / %x20 / %x21 / %x23-7E ; TAB / SPACE / all printable characters except `"`
/// COMMA = ","
/// DOT = "."
/// EQUAL = "="
///
/// ALPHA = %x41-5A / %x61-7A ; A-Z / a-z
/// DIGIT = %x30-39
/// WS = %x09 / " "
/// ```
pub(crate) struct TagIterator<'a, 'tcx> {
    inner: Peekable<CharIndices<'a>>,
    data: &'a str,
    is_in_attribute_block: bool,
    extra: Option<&'a ExtraInfo<'tcx>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum LangStringToken<'a> {
    LangToken(&'a str),
    ClassAttribute(&'a str),
    KeyValueAttribute(&'a str, &'a str),
}

fn is_leading_char(c: char) -> bool {
    c == '_' || c == '-' || c == ':' || c.is_ascii_alphabetic() || c.is_ascii_digit()
}
fn is_bareword_char(c: char) -> bool {
    is_leading_char(c) || ".!#$%&*+/;<>?@^|~".contains(c)
}
fn is_separator(c: char) -> bool {
    c == ' ' || c == ',' || c == '\t'
}

struct Indices {
    start: usize,
    end: usize,
}

impl<'a, 'tcx> TagIterator<'a, 'tcx> {
    pub(crate) fn new(data: &'a str, extra: Option<&'a ExtraInfo<'tcx>>) -> Self {
        Self {
            inner: data.char_indices().peekable(),
            data,
            is_in_attribute_block: false,
            extra,
        }
    }

    fn emit_error(&self, err: impl Into<DiagnosticMessage>) {
        if let Some(extra) = self.extra {
            extra.error_invalid_codeblock_attr(err);
        }
    }

    fn skip_separators(&mut self) -> Option<usize> {
        while let Some((pos, c)) = self.inner.peek() {
            if !is_separator(*c) {
                return Some(*pos);
            }
            self.inner.next();
        }
        None
    }

    fn parse_string(&mut self, start: usize) -> Option<Indices> {
        while let Some((pos, c)) = self.inner.next() {
            if c == '"' {
                return Some(Indices {
                    start: start + 1,
                    end: pos,
                });
            }
        }
        self.emit_error("unclosed quote string `\"`");
        None
    }

    fn parse_class(&mut self, start: usize) -> Option<LangStringToken<'a>> {
        while let Some((pos, c)) = self.inner.peek().copied() {
            if is_bareword_char(c) {
                self.inner.next();
            } else {
                let class = &self.data[start + 1..pos];
                if class.is_empty() {
                    self.emit_error(format!("unexpected `{c}` character after `.`"));
                    return None;
                } else if self.check_after_token() {
                    return Some(LangStringToken::ClassAttribute(class));
                } else {
                    return None;
                }
            }
        }
        let class = &self.data[start + 1..];
        if class.is_empty() {
            self.emit_error("missing character after `.`");
            None
        } else if self.check_after_token() {
            Some(LangStringToken::ClassAttribute(class))
        } else {
            None
        }
    }

    fn parse_token(&mut self, start: usize) -> Option<Indices> {
        while let Some((pos, c)) = self.inner.peek() {
            if !is_bareword_char(*c) {
                return Some(Indices { start, end: *pos });
            }
            self.inner.next();
        }
        self.emit_error("unexpected end");
        None
    }

    fn parse_key_value(&mut self, c: char, start: usize) -> Option<LangStringToken<'a>> {
        let key_indices = if c == '"' {
            self.parse_string(start)?
        } else {
            self.parse_token(start)?
        };
        if key_indices.start == key_indices.end {
            self.emit_error("unexpected empty string as key");
            return None;
        }

        if let Some((_, c)) = self.inner.next() {
            if c != '=' {
                self.emit_error(format!("expected `=`, found `{}`", c));
                return None;
            }
        } else {
            self.emit_error("unexpected end");
            return None;
        }
        let value_indices = match self.inner.next() {
            Some((pos, '"')) => self.parse_string(pos)?,
            Some((pos, c)) if is_bareword_char(c) => self.parse_token(pos)?,
            Some((_, c)) => {
                self.emit_error(format!("unexpected `{c}` character after `=`"));
                return None;
            }
            None => {
                self.emit_error("expected value after `=`");
                return None;
            }
        };
        if value_indices.start == value_indices.end {
            self.emit_error("unexpected empty string as value");
            None
        } else if self.check_after_token() {
            Some(LangStringToken::KeyValueAttribute(
                &self.data[key_indices.start..key_indices.end],
                &self.data[value_indices.start..value_indices.end],
            ))
        } else {
            None
        }
    }

    /// Returns `false` if an error was emitted.
    fn check_after_token(&mut self) -> bool {
        if let Some((_, c)) = self.inner.peek().copied() {
            if c == '}' || is_separator(c) || c == '(' {
                true
            } else {
                self.emit_error(format!("unexpected `{c}` character"));
                false
            }
        } else {
            // The error will be caught on the next iteration.
            true
        }
    }

    fn parse_in_attribute_block(&mut self) -> Option<LangStringToken<'a>> {
        if let Some((pos, c)) = self.inner.next() {
            if c == '}' {
                self.is_in_attribute_block = false;
                return self.next();
            } else if c == '.' {
                return self.parse_class(pos);
            } else if c == '"' || is_leading_char(c) {
                return self.parse_key_value(c, pos);
            } else {
                self.emit_error(format!("unexpected character `{c}`"));
                return None;
            }
        }
        self.emit_error("unclosed attribute block (`{}`): missing `}` at the end");
        None
    }

    /// Returns `false` if an error was emitted.
    fn skip_paren_block(&mut self) -> bool {
        while let Some((_, c)) = self.inner.next() {
            if c == ')' {
                return true;
            }
        }
        self.emit_error("unclosed comment: missing `)` at the end");
        false
    }

    fn parse_outside_attribute_block(&mut self, start: usize) -> Option<LangStringToken<'a>> {
        while let Some((pos, c)) = self.inner.next() {
            if c == '"' {
                if pos != start {
                    self.emit_error("expected ` `, `{` or `,` found `\"`");
                    return None;
                }
                let indices = self.parse_string(pos)?;
                if let Some((_, c)) = self.inner.peek().copied()
                    && c != '{'
                    && !is_separator(c)
                    && c != '('
                {
                    self.emit_error(format!("expected ` `, `{{` or `,` after `\"`, found `{c}`"));
                    return None;
                }
                return Some(LangStringToken::LangToken(
                    &self.data[indices.start..indices.end],
                ));
            } else if c == '{' {
                self.is_in_attribute_block = true;
                return self.next();
            } else if is_separator(c) {
                if pos != start {
                    return Some(LangStringToken::LangToken(&self.data[start..pos]));
                }
                return self.next();
            } else if c == '(' {
                if !self.skip_paren_block() {
                    return None;
                }
                if pos != start {
                    return Some(LangStringToken::LangToken(&self.data[start..pos]));
                }
                return self.next();
            } else if pos == start && is_leading_char(c) {
                continue;
            } else if pos != start && is_bareword_char(c) {
                continue;
            } else {
                self.emit_error(format!("unexpected character `{c}`"));
                return None;
            }
        }
        let token = &self.data[start..];
        if token.is_empty() {
            None
        } else {
            Some(LangStringToken::LangToken(&self.data[start..]))
        }
    }
}

impl<'a, 'tcx> Iterator for TagIterator<'a, 'tcx> {
    type Item = LangStringToken<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let Some(start) = self.skip_separators() else {
            if self.is_in_attribute_block {
                self.emit_error("unclosed attribute block (`{}`): missing `}` at the end");
            }
            return None;
        };
        if self.is_in_attribute_block {
            self.parse_in_attribute_block()
        } else {
            self.parse_outside_attribute_block(start)
        }
    }
}

fn tokens(string: &str) -> impl Iterator<Item = LangStringToken<'_>> {
    // Pandoc, which Rust once used for generating documentation,
    // expects lang strings to be surrounded by `{}` and for each token
    // to be proceeded by a `.`. Since some of these lang strings are still
    // loose in the wild, we strip a pair of surrounding `{}` from the lang
    // string and a leading `.` from each token.

    let string = string.trim();

    let first = string.chars().next();
    let last = string.chars().last();

    let string = if first == Some('{') && last == Some('}') {
        &string[1..string.len() - 1]
    } else {
        string
    };

    string
        .split(|c| c == ',' || c == ' ' || c == '\t')
        .map(str::trim)
        .map(|token| token.strip_prefix('.').unwrap_or(token))
        .filter(|token| !token.is_empty())
        .map(|token| LangStringToken::LangToken(token))
}

//#####################################################################################################################
// Source: https://doc.rust-lang.org/1.77.0/nightly-rustc/src/rustdoc/html/markdown.rs.html#140-182

/// Controls whether a line will be hidden or shown in HTML output.
///
/// All lines are used in documentation tests.
enum Line<'a> {
    Hidden(&'a str),
    Shown(Cow<'a, str>),
}

impl<'a> Line<'a> {
    fn for_html(self) -> Option<Cow<'a, str>> {
        match self {
            Line::Shown(l) => Some(l),
            Line::Hidden(_) => None,
        }
    }

    fn for_code(self) -> Cow<'a, str> {
        match self {
            Line::Shown(l) => l,
            Line::Hidden(l) => Cow::Borrowed(l),
        }
    }
}

// FIXME: There is a minor inconsistency here. For lines that start with ##, we
// have no easy way of removing a potential single space after the hashes, which
// is done in the single # case. This inconsistency seems okay, if non-ideal. In
// order to fix it we'd have to iterate to find the first non-# character, and
// then reallocate to remove it; which would make us return a String.
fn map_line(s: &str) -> Line<'_> {
    let trimmed = s.trim();
    if trimmed.starts_with("##") {
        Line::Shown(Cow::Owned(s.replacen("##", "#", 1)))
    } else if let Some(stripped) = trimmed.strip_prefix("# ") {
        // # text
        Line::Hidden(stripped)
    } else if trimmed == "#" {
        // We cannot handle '#text' because it could be #[attr].
        Line::Hidden("")
    } else {
        Line::Shown(Cow::Borrowed(s))
    }
}

//#####################################################################################################################
// Source: https://doc.rust-lang.org/1.77.0/nightly-rustc/src/rustdoc/doctest.rs.html#220-248

// Look for `#![doc(test(no_crate_inject))]`, used by crates in the std facade.
pub(crate) fn scrape_test_config(attrs: &[ast::Attribute]) -> GlobalTestOptions {
    use rustc_ast_pretty::pprust;

    let mut opts = GlobalTestOptions {
        no_crate_inject: false,
        attrs: Vec::new(),
    };

    let test_attrs: Vec<_> = attrs
        .iter()
        .filter(|a| a.has_name(rustc_span::sym::doc))
        .flat_map(|a| a.meta_item_list().unwrap_or_default())
        .filter(|a| a.has_name(rustc_span::sym::test))
        .collect();
    let attrs = test_attrs
        .iter()
        .flat_map(|a| a.meta_item_list().unwrap_or(&[]));

    for attr in attrs {
        if attr.has_name(rustc_span::sym::no_crate_inject) {
            opts.no_crate_inject = true;
        }
        if attr.has_name(rustc_span::sym::attr)
            && let Some(l) = attr.meta_item_list()
        {
            for item in l {
                opts.attrs.push(pprust::meta_list_item_to_string(item));
            }
        }
    }

    opts
}

//#####################################################################################################################
// Source: https://doc.rust-lang.org/1.77.0/nightly-rustc/src/rustdoc/doctest.rs.html#38-45

/// Options that apply to all doctests in a crate or Markdown file (for `rustdoc foo.md`).
#[derive(Clone, Default)]
pub(crate) struct GlobalTestOptions {
    /// Whether to disable the default `extern crate my_crate;` when creating doctests.
    pub(crate) no_crate_inject: bool,
    /// Additional crate-level attributes to add to doctests.
    pub(crate) attrs: Vec<String>,
}
