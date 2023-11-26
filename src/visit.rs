// Copyright 2021-2023 Martin Pool

//! Visit all the files in a source tree, and then the AST of each file,
//! to discover mutation opportunities.
//!
//! Walking the tree starts with some root files known to the build tool:
//! e.g. for cargo they are identified from the targets. The tree walker then
//! follows `mod` statements to recursively visit other referenced files.

use std::collections::VecDeque;
use std::sync::Arc;

use anyhow::Context;
use itertools::Itertools;
use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::ext::IdentExt;
use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{Attribute, BinOp, Block, Expr, ItemFn, ReturnType, Signature};
use tracing::{debug, debug_span, trace, trace_span, warn};

use crate::fnvalue::return_type_replacements;
use crate::mutate::Function;
use crate::pretty::ToPrettyString;
use crate::source::SourceFile;
use crate::textedit::Span;
use crate::*;

/// Mutants and files discovered in a source tree.
///
/// Files are listed separately so that we can represent files that
/// were visited but that produced no mutants.
pub struct Discovered {
    pub mutants: Vec<Mutant>,
    pub files: Vec<Arc<SourceFile>>,
}

/// Discover all mutants and all source files.
///
/// The list of source files includes even those with no mutants.
///
pub fn walk_tree(
    workspace_dir: &Utf8Path,
    top_source_files: &[Arc<SourceFile>],
    options: &Options,
    console: &Console,
) -> Result<Discovered> {
    // TODO: Lift up parsing the error expressions...
    let error_exprs = options
        .error_values
        .iter()
        .map(|e| syn::parse_str(e).with_context(|| format!("Failed to parse error value {e:?}")))
        .collect::<Result<Vec<Expr>>>()?;
    console.walk_tree_start();
    let mut file_queue: VecDeque<Arc<SourceFile>> = top_source_files.iter().cloned().collect();
    let mut mutants = Vec::new();
    let mut files: Vec<Arc<SourceFile>> = Vec::new();
    while let Some(source_file) = file_queue.pop_front() {
        console.walk_tree_update(files.len(), mutants.len());
        check_interrupted()?;
        let (mut file_mutants, external_mods) = walk_file(Arc::clone(&source_file), &error_exprs)?;
        // We'll still walk down through files that don't match globs, so that
        // we have a chance to find modules underneath them. However, we won't
        // collect any mutants from them, and they don't count as "seen" for
        // `--list-files`.
        for mod_name in &external_mods {
            if let Some(mod_path) = find_mod_source(workspace_dir, &source_file, mod_name)? {
                file_queue.push_back(Arc::new(SourceFile::new(
                    workspace_dir,
                    mod_path,
                    &source_file.package,
                )?))
            }
        }
        let path = &source_file.tree_relative_path;
        if let Some(examine_globset) = &options.examine_globset {
            if !examine_globset.is_match(path) {
                trace!("{path:?} does not match examine globset");
                continue;
            }
        }
        if let Some(exclude_globset) = &options.exclude_globset {
            if exclude_globset.is_match(path) {
                trace!("{path:?} excluded by globset");
                continue;
            }
        }
        if !options.examine_names.is_empty() {
            file_mutants.retain(|m| options.examine_names.is_match(&m.to_string()));
        }
        if !options.exclude_names.is_empty() {
            file_mutants.retain(|m| !options.exclude_names.is_match(&m.to_string()));
        }
        mutants.append(&mut file_mutants);
        files.push(source_file);
    }
    console.walk_tree_done();
    Ok(Discovered { mutants, files })
}

/// Find all possible mutants in a source file.
///
/// Returns the mutants found, and the names of modules referenced by `mod` statements
/// that should be visited later.
fn walk_file(
    source_file: Arc<SourceFile>,
    error_exprs: &[Expr],
) -> Result<(Vec<Mutant>, Vec<String>)> {
    let _span = debug_span!("source_file", path = source_file.tree_relative_slashes()).entered();
    debug!("visit source file");
    let syn_file = syn::parse_str::<syn::File>(&source_file.code)
        .with_context(|| format!("failed to parse {}", source_file.tree_relative_slashes()))?;
    let mut visitor = DiscoveryVisitor {
        error_exprs,
        external_mods: Vec::new(),
        mutants: Vec::new(),
        namespace_stack: Vec::new(),
        fn_stack: Vec::new(),
        source_file: Arc::clone(&source_file),
    };
    visitor.visit_file(&syn_file);
    Ok((visitor.mutants, visitor.external_mods))
}

/// `syn` visitor that recursively traverses the syntax tree, accumulating places
/// that could be mutated.
///
/// As it walks the tree, it accumulates within itself a list of mutation opportunities,
/// and other files referenced by `mod` statements that should be visited later.
struct DiscoveryVisitor<'o> {
    /// All the mutants generated by visiting the file.
    mutants: Vec<Mutant>,

    /// The file being visited.
    source_file: Arc<SourceFile>,

    /// The stack of namespaces we're currently inside.
    namespace_stack: Vec<String>,

    /// The functions we're inside.
    fn_stack: Vec<Arc<Function>>,

    /// The names from `mod foo;` statements that should be visited later.
    external_mods: Vec<String>,

    /// Parsed error expressions, from the config file or command line.
    error_exprs: &'o [Expr],
}

impl<'o> DiscoveryVisitor<'o> {
    fn current_function(&self) -> Arc<Function> {
        Arc::clone(
            self.fn_stack
                .last()
                .expect("Function stack should not be empty"),
        )
    }

    fn enter_function(
        &mut self,
        function_name: &Ident,
        return_type: &ReturnType,
        span: proc_macro2::Span,
    ) -> Arc<Function> {
        self.namespace_stack.push(function_name.to_string());
        let function_name = self.namespace_stack.join("::");
        let function = Arc::new(Function {
            function_name: function_name.to_owned(),
            return_type: return_type.to_pretty_string(),
            span: span.into(),
        });
        self.fn_stack.push(Arc::clone(&function));
        function
    }

    fn leave_function(&mut self, function: Arc<Function>) {
        self.namespace_stack
            .pop()
            .expect("Namespace stack should not be empty");
        assert_eq!(
            self.fn_stack.pop(),
            Some(function),
            "Function stack mismatch"
        );
    }

    fn collect_fn_mutants(&mut self, sig: &Signature, block: &Block) {
        let function = self.current_function();
        let body_span = function_body_span(block).expect("Empty function body");
        let mut new_mutants = return_type_replacements(&sig.output, self.error_exprs)
            .map(|rep| Mutant {
                source_file: Arc::clone(&self.source_file),
                function: Arc::clone(&function),
                span: body_span,
                replacement: rep.to_pretty_string(),
                primary_line: sig.span().start().line,
                genre: Genre::FnValue,
            })
            .collect_vec();
        if new_mutants.is_empty() {
            debug!(
                function_name = function.function_name,
                return_type = function.return_type,
                "No mutants generated for this return type"
            );
        } else {
            self.mutants.append(&mut new_mutants);
        }
    }

    /// Call a function with a namespace pushed onto the stack.
    ///
    /// This is used when recursively descending into a namespace.
    fn in_namespace<F, T>(&mut self, name: &str, f: F) -> T
    where
        F: FnOnce(&mut Self) -> T,
    {
        self.namespace_stack.push(name.to_owned());
        let r = f(self);
        assert_eq!(self.namespace_stack.pop().unwrap(), name);
        r
    }
}

impl<'ast> Visit<'ast> for DiscoveryVisitor<'_> {
    /// Visit top-level `fn foo()`.
    fn visit_item_fn(&mut self, i: &'ast ItemFn) {
        let function_name = i.sig.ident.to_pretty_string();
        let _span = trace_span!(
            "fn",
            line = i.sig.fn_token.span.start().line,
            name = function_name
        )
        .entered();
        if fn_sig_excluded(&i.sig) || attrs_excluded(&i.attrs) || block_is_empty(&i.block) {
            return;
        }
        let function = self.enter_function(&i.sig.ident, &i.sig.output, i.span());
        self.collect_fn_mutants(&i.sig, &i.block);
        syn::visit::visit_item_fn(self, i);
        self.leave_function(function);
    }

    /// Visit `fn foo()` within an `impl`.
    fn visit_impl_item_fn(&mut self, i: &'ast syn::ImplItemFn) {
        // Don't look inside constructors (called "new") because there's often no good
        // alternative.
        let function_name = i.sig.ident.to_pretty_string();
        let _span = trace_span!(
            "fn",
            line = i.sig.fn_token.span.start().line,
            name = function_name
        )
        .entered();
        if fn_sig_excluded(&i.sig)
            || attrs_excluded(&i.attrs)
            || i.sig.ident == "new"
            || block_is_empty(&i.block)
        {
            return;
        }
        let function = self.enter_function(&i.sig.ident, &i.sig.output, i.span());
        self.collect_fn_mutants(&i.sig, &i.block);
        syn::visit::visit_impl_item_fn(self, i);
        self.leave_function(function);
    }

    /// Visit `impl Foo { ...}` or `impl Debug for Foo { ... }`.
    fn visit_item_impl(&mut self, i: &'ast syn::ItemImpl) {
        if attrs_excluded(&i.attrs) {
            return;
        }
        let type_name = i.self_ty.to_pretty_string();
        let name = if let Some((_, trait_path, _)) = &i.trait_ {
            let trait_name = &trait_path.segments.last().unwrap().ident;
            if trait_name == "Default" {
                // Can't think of how to generate a viable different default.
                return;
            }
            format!("<impl {trait_name} for {type_name}>")
        } else {
            type_name
        };
        self.in_namespace(&name, |v| syn::visit::visit_item_impl(v, i));
    }

    /// Visit `mod foo { ... }` or `mod foo;`.
    fn visit_item_mod(&mut self, node: &'ast syn::ItemMod) {
        let mod_name = &node.ident.unraw().to_string();
        let _span = trace_span!("mod", line = node.mod_token.span.start().line, mod_name).entered();
        if attrs_excluded(&node.attrs) {
            trace!("mod excluded by attrs");
            return;
        }
        // If there's no content in braces, then this is a `mod foo;`
        // statement referring to an external file. We remember the module
        // name and then later look for the file.
        if node.content.is_none() {
            self.external_mods.push(mod_name.to_owned());
        }
        self.in_namespace(mod_name, |v| syn::visit::visit_item_mod(v, node));
    }

    /// Visit `a op b` expressions.
    fn visit_expr_binary(&mut self, i: &'ast syn::ExprBinary) {
        let _span = trace_span!("binary", line = i.op.span().start().line).entered();
        if attrs_excluded(&i.attrs) {
            return;
        }
        let mut new_mutants = binary_operator_replacements(i.op)
            .into_iter()
            .map(|rep| Mutant {
                source_file: Arc::clone(&self.source_file),
                function: self.current_function(),
                replacement: rep.to_pretty_string(),
                span: i.op.span().into(),
                primary_line: i.op.span().start().line,
                genre: Genre::BinaryOperator,
            })
            .collect_vec();
        if new_mutants.is_empty() {
            debug!(
                op = i.op.to_pretty_string(),
                "No mutants generated for this binary operator"
            );
        } else {
            self.mutants.append(&mut new_mutants);
        }
        syn::visit::visit_expr_binary(self, i);
    }
}

// Get the span of the block excluding the braces, or None if it is empty.
fn function_body_span(block: &Block) -> Option<Span> {
    let start = block.stmts.first()?.span().start();
    let end = block.stmts.last()?.span().end();
    Some(Span {
        start: start.into(),
        end: end.into(),
    })
}

fn binary_operator_replacements(op: syn::BinOp) -> Vec<TokenStream> {
    match op {
        // We don't generate `<=` from `==` because it can too easily go
        // wrong with unsigned types compared to 0.
        BinOp::Eq(_) => vec![
            quote! { != },
            // (quote! { > }, Genre::BinaryOperator),
            // (quote! { < }, Genre::BinaryOperator),
            // (quote! { >= }, Genre::BinaryOperator),
            // (quote! { <= }, Genre::BinaryOperator),
        ],
        _ => Vec::new(),
    }
}

/// Find a new source file referenced by a `mod` statement.
///
/// Possibly, our heuristics just won't be able to find which file it is,
/// in which case we return `Ok(None)`.
fn find_mod_source(
    tree_root: &Utf8Path,
    parent: &SourceFile,
    mod_name: &str,
) -> Result<Option<Utf8PathBuf>> {
    // Both the current module and the included sub-module can be in
    // either style: `.../foo.rs` or `.../foo/mod.rs`.
    //
    // If the current file ends with `/mod.rs`, then sub-modules
    // will be in the same directory as this file. Otherwise, this is
    // `/foo.rs` and sub-modules will be in `foo/`.
    //
    // Having determined the directory then we can look for either
    // `foo.rs` or `foo/mod.rs`.
    let parent_path = &parent.tree_relative_path;
    // TODO: Maybe matching on the name here is not the right approach and
    // we should instead remember how this file was found? This might go wrong
    // with unusually-named files.
    let dir = if parent_path.ends_with("mod.rs")
        || parent_path.ends_with("lib.rs")
        || parent_path.ends_with("main.rs")
    {
        parent_path
            .parent()
            .expect("mod path has no parent")
            .to_owned()
    } else {
        parent_path.with_extension("")
    };
    let mut tried_paths = Vec::new();
    for &tail in &[".rs", "/mod.rs"] {
        let relative_path = dir.join(mod_name.to_owned() + tail);
        let full_path = tree_root.join(&relative_path);
        if full_path.is_file() {
            trace!("found submodule in {full_path}");
            return Ok(Some(relative_path));
        } else {
            tried_paths.push(full_path);
        }
    }
    warn!(?parent_path, %mod_name, ?tried_paths, "referent of mod not found");
    Ok(None)
}

/// True if the signature of a function is such that it should be excluded.
fn fn_sig_excluded(sig: &syn::Signature) -> bool {
    if sig.unsafety.is_some() {
        trace!("Skip unsafe fn");
        true
    } else {
        false
    }
}

/// True if any of the attrs indicate that we should skip this node and everything inside it.
fn attrs_excluded(attrs: &[Attribute]) -> bool {
    attrs
        .iter()
        .any(|attr| attr_is_cfg_test(attr) || attr_is_test(attr) || attr_is_mutants_skip(attr))
}

/// True if the block (e.g. the contents of a function) is empty.
fn block_is_empty(block: &syn::Block) -> bool {
    block.stmts.is_empty()
}

/// True if the attribute looks like `#[cfg(test)]`, or has "test"
/// anywhere in it.
fn attr_is_cfg_test(attr: &Attribute) -> bool {
    if !path_is(attr.path(), &["cfg"]) {
        return false;
    }
    let mut contains_test = false;
    if let Err(err) = attr.parse_nested_meta(|meta| {
        if meta.path.is_ident("test") {
            contains_test = true;
        }
        Ok(())
    }) {
        debug!(
            ?err,
            ?attr,
            "Attribute is not in conventional form; skipped"
        );
        return false;
    }
    contains_test
}

/// True if the attribute is `#[test]`.
fn attr_is_test(attr: &Attribute) -> bool {
    attr.path().is_ident("test")
}

fn path_is(path: &syn::Path, idents: &[&str]) -> bool {
    path.segments.iter().map(|ps| &ps.ident).eq(idents.iter())
}

/// True if the attribute contains `mutants::skip`.
///
/// This for example returns true for `#[mutants::skip] or `#[cfg_attr(test, mutants::skip)]`.
fn attr_is_mutants_skip(attr: &Attribute) -> bool {
    if path_is(attr.path(), &["mutants", "skip"]) {
        return true;
    }
    if !path_is(attr.path(), &["cfg_attr"]) {
        return false;
    }
    let mut skip = false;
    if let Err(err) = attr.parse_nested_meta(|meta| {
        if path_is(&meta.path, &["mutants", "skip"]) {
            skip = true
        }
        Ok(())
    }) {
        debug!(
            ?attr,
            ?err,
            "Attribute is not a path with attributes; skipping"
        );
        return false;
    }
    skip
}

#[cfg(test)]
mod test {
    use regex::Regex;

    use super::*;

    /// As a generic protection against regressions in discovery, the the mutants
    /// generated from `cargo-mutants` own tree against a checked-in list.
    ///
    /// The snapshot will need to be updated when functions are added or removed,
    /// as well as when new mutation patterns are added.
    ///
    /// To stop it being too noisy, we use a custom format with no line numbers.
    #[test]
    fn expected_mutants_for_own_source_tree() {
        let options = Options {
            error_values: vec!["::anyhow::anyhow!(\"mutated!\")".to_owned()],
            ..Default::default()
        };
        let mut list_output = String::new();
        let console = Console::new();
        let workspace = Workspace::open(
            Utf8Path::new(".")
                .canonicalize_utf8()
                .expect("Canonicalize source path"),
        )
        .unwrap();
        let discovered = workspace
            .discover(&PackageFilter::All, &options, &console)
            .expect("Discover mutants");
        crate::list_mutants(&mut list_output, &discovered.mutants, &options)
            .expect("Discover mutants in own source tree");

        // Strip line numbers so this is not too brittle.
        let line_re = Regex::new(r"(?m)^([^:]+:)\d+:( .*)$").unwrap();
        let list_output = line_re.replace_all(&list_output, "$1$2");
        insta::assert_snapshot!(list_output);
    }
}
