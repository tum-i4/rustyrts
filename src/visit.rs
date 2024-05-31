// Copyright 2021-2024 Martin Pool

//! Visit all the files in a source tree, and then the AST of each file,
//! to discover mutation opportunities.
//!
//! Walking the tree starts with some root files known to the build tool:
//! e.g. for cargo they are identified from the targets. The tree walker then
//! follows `mod` statements to recursively visit other referenced files.

use std::collections::VecDeque;
use std::sync::Arc;
use std::vec;

use anyhow::Context;
use proc_macro2::{Ident, TokenStream};
use quote::{quote, ToTokens};
use syn::ext::IdentExt;
use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{Attribute, BinOp, Block, Expr, ItemFn, ReturnType, Signature};
use tracing::{debug, debug_span, trace, trace_span, warn};

use crate::fnvalue::return_type_replacements;
use crate::mutate::Function;
use crate::pretty::ToPrettyString;
use crate::source::SourceFile;
use crate::span::Span;
use crate::*;

/// Mutants and files discovered in a source tree.
///
/// Files are listed separately so that we can represent files that
/// were visited but that produced no mutants.
pub struct Discovered {
    pub mutants: Vec<Mutant>,
    pub files: Vec<SourceFile>,
}

/// Discover all mutants and all source files.
///
/// The list of source files includes even those with no mutants.
///
pub fn walk_tree(
    workspace_dir: &Utf8Path,
    top_source_files: &[SourceFile],
    options: &Options,
    console: &Console,
) -> Result<Discovered> {
    let error_exprs = options
        .error_values
        .iter()
        .map(|e| syn::parse_str(e).with_context(|| format!("Failed to parse error value {e:?}")))
        .collect::<Result<Vec<Expr>>>()?;
    console.walk_tree_start();
    let mut file_queue: VecDeque<SourceFile> = top_source_files.iter().cloned().collect();
    let mut files_visited: Vec<Utf8PathBuf> = Vec::new();
    let mut mutants = Vec::new();
    let mut files: Vec<SourceFile> = Vec::new();
    while let Some(source_file) = file_queue.pop_front() {
        console.walk_tree_update(files.len(), mutants.len());
        check_interrupted()?;
        let (mut file_mutants, external_mods) = walk_file(&source_file, &error_exprs)?;
        // We'll still walk down through files that don't match globs, so that
        // we have a chance to find modules underneath them. However, we won't
        // collect any mutants from them, and they don't count as "seen" for
        // `--list-files`.
        for mod_name in &external_mods {
            if let Some(mod_path) = find_mod_source(workspace_dir, &source_file, mod_name)? {
                if !files_visited.contains(&mod_path) {
                    files_visited.push(mod_path.clone());
                    file_queue.push_back(SourceFile::new(
                        workspace_dir,
                        mod_path,
                        &source_file.package,
                        false,
                    )?)
                }
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
        mutants.append(&mut file_mutants);
        files.push(source_file);
    }
    mutants.retain(|m| {
        let name = m.name(true, false);
        (options.examine_names.is_empty() || options.examine_names.is_match(&name))
            && (options.exclude_names.is_empty() || !options.exclude_names.is_match(&name))
    });
    console.walk_tree_done();
    Ok(Discovered { mutants, files })
}

/// Find all possible mutants in a source file.
///
/// Returns the mutants found, and the names of modules referenced by `mod` statements
/// that should be visited later.
fn walk_file(source_file: &SourceFile, error_exprs: &[Expr]) -> Result<(Vec<Mutant>, Vec<String>)> {
    let _span = debug_span!("source_file", path = source_file.tree_relative_slashes()).entered();
    debug!("visit source file");
    let syn_file = syn::parse_str::<syn::File>(source_file.code())
        .with_context(|| format!("failed to parse {}", source_file.tree_relative_slashes()))?;
    let mut visitor = DiscoveryVisitor {
        error_exprs,
        external_mods: Vec::new(),
        mutants: Vec::new(),
        namespace_stack: Vec::new(),
        fn_stack: Vec::new(),
        source_file: source_file.clone(),
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
    source_file: SourceFile,

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

    /// Record that we generated some mutants.
    fn collect_mutant(&mut self, span: Span, replacement: TokenStream, genre: Genre) {
        self.mutants.push(Mutant {
            source_file: self.source_file.clone(),
            function: self.fn_stack.last().map(Arc::clone),
            span,
            replacement: replacement.to_pretty_string(),
            genre,
        })
    }

    fn collect_fn_mutants(&mut self, sig: &Signature, block: &Block) {
        if let Some(function) = self.fn_stack.last().map(Arc::clone) {
            let body_span = function_body_span(block).expect("Empty function body");
            let repls = return_type_replacements(&sig.output, self.error_exprs);
            if repls.is_empty() {
                debug!(
                    function_name = function.function_name,
                    return_type = function.return_type,
                    "No mutants generated for this return type"
                );
            } else {
                let orig_block = block.to_token_stream().to_pretty_string();
                for rep in repls {
                    // Comparing strings is a kludge for proc_macro2 not (yet) apparently
                    // exposing any way to compare token streams...
                    //
                    // TODO: Maybe this should move into collect_mutant, but at the moment
                    // FnValue is the only genre that seems able to generate no-ops.
                    //
                    // The original block has braces and the replacements don't, so put
                    // them back for the comparison...
                    let new_block = quote!( { #rep } ).to_token_stream().to_pretty_string();
                    // dbg!(&orig_block, &new_block);
                    if orig_block == new_block {
                        debug!("Replacement is the same as the function body; skipping");
                    } else {
                        self.collect_mutant(body_span, rep, Genre::FnValue);
                    }
                }
            }
        } else {
            warn!("collect_fn_mutants called while not in a function?");
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
        trace!("visit fn");
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

    /// Visit `fn foo() { ... }` within a trait, i.e. a default implementation of a function.
    fn visit_trait_item_fn(&mut self, i: &'ast syn::TraitItemFn) {
        let function_name = i.sig.ident.to_pretty_string();
        let _span = trace_span!(
            "fn",
            line = i.sig.fn_token.span.start().line,
            name = function_name
        )
        .entered();
        if fn_sig_excluded(&i.sig) || attrs_excluded(&i.attrs) || i.sig.ident == "new" {
            return;
        }
        if let Some(block) = &i.default {
            if block_is_empty(block) {
                return;
            }
            let function = self.enter_function(&i.sig.ident, &i.sig.output, i.span());
            self.collect_fn_mutants(&i.sig, block);
            syn::visit::visit_trait_item_fn(self, i);
            self.leave_function(function);
        }
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

    /// Visit `trait Foo { ... }`
    fn visit_item_trait(&mut self, i: &'ast syn::ItemTrait) {
        let name = i.ident.to_pretty_string();
        let _span = trace_span!("trait", line = i.span().start().line, name).entered();
        if attrs_excluded(&i.attrs) {
            return;
        }
        self.in_namespace(&name, |v| syn::visit::visit_item_trait(v, i));
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
        trace!("visit binary operator");
        if attrs_excluded(&i.attrs) {
            return;
        }
        let replacements = match i.op {
            // We don't generate `<=` from `==` because it can too easily go
            // wrong with unsigned types compared to 0.

            // We try replacing logical ops with == and !=, which are effectively
            // XNOR and XOR when applied to booleans. However, they're often unviable
            // because they require parenthesis for disambiguation in many expressions.
            BinOp::Eq(_) => vec![quote! { != }],
            BinOp::Ne(_) => vec![quote! { == }],
            BinOp::And(_) => vec![quote! { || }],
            BinOp::Or(_) => vec![quote! { && }],
            BinOp::Lt(_) => vec![quote! { == }, quote! {>}],
            BinOp::Gt(_) => vec![quote! { == }, quote! {<}],
            BinOp::Le(_) => vec![quote! {>}],
            BinOp::Ge(_) => vec![quote! {<}],
            BinOp::Add(_) => vec![quote! {-}, quote! {*}],
            BinOp::AddAssign(_) => vec![quote! {-=}, quote! {*=}],
            BinOp::Sub(_) => vec![quote! {+}, quote! {/}],
            BinOp::SubAssign(_) => vec![quote! {+=}, quote! {/=}],
            BinOp::Mul(_) => vec![quote! {+}, quote! {/}],
            BinOp::MulAssign(_) => vec![quote! {+=}, quote! {/=}],
            BinOp::Div(_) => vec![quote! {%}, quote! {*}],
            BinOp::DivAssign(_) => vec![quote! {%=}, quote! {*=}],
            BinOp::Rem(_) => vec![quote! {/}, quote! {+}],
            BinOp::RemAssign(_) => vec![quote! {/=}, quote! {+=}],
            BinOp::Shl(_) => vec![quote! {>>}],
            BinOp::ShlAssign(_) => vec![quote! {>>=}],
            BinOp::Shr(_) => vec![quote! {<<}],
            BinOp::ShrAssign(_) => vec![quote! {<<=}],
            BinOp::BitAnd(_) => vec![quote! {|}, quote! {^}],
            BinOp::BitAndAssign(_) => vec![quote! {|=}, quote! {^=}],
            BinOp::BitOr(_) => vec![quote! {&}, quote! {^}],
            BinOp::BitOrAssign(_) => vec![quote! {&=}, quote! {^=}],
            BinOp::BitXor(_) => vec![quote! {|}, quote! {&}],
            BinOp::BitXorAssign(_) => vec![quote! {|=}, quote! {&=}],
            _ => {
                trace!(
                    op = i.op.to_pretty_string(),
                    "No mutants generated for this binary operator"
                );
                Vec::new()
            }
        };
        replacements
            .into_iter()
            .for_each(|rep| self.collect_mutant(i.op.span().into(), rep, Genre::BinaryOperator));
        syn::visit::visit_expr_binary(self, i);
    }
}

// Get the span of the block excluding the braces, or None if it is empty.
fn function_body_span(block: &Block) -> Option<Span> {
    Some(Span {
        start: block.stmts.first()?.span().start().into(),
        end: block.stmts.last()?.span().end().into(),
    })
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
    // First, work out whether the mod will be a sibling in the same directory, or
    // in a child directory.
    //
    // 1. The parent is "src/foo.rs" and `mod bar` means "src/foo/bar.rs".
    //
    // 2. The parent is "src/lib.rs" (a target top file) and `mod bar` means "src/bar.rs".
    //
    // 3. The parent is "src/foo/mod.rs" and so `mod bar` means "src/foo/bar.rs".
    //
    // Having determined the right directory then we can look for either
    // `foo.rs` or `foo/mod.rs`.

    // TODO: Beyond #115, we should probably remove all special handling of
    // `mod.rs` here by remembering how we found this file, and whether it
    // is above or inside the directory corresponding to its module?

    let parent_path = &parent.tree_relative_path;
    let search_dir = if parent.is_top || parent_path.ends_with("mod.rs") {
        parent_path
            .parent()
            .expect("mod path has no parent")
            .to_owned() // src/lib.rs -> src/
    } else {
        parent_path.with_extension("") // foo.rs -> foo/
    };
    let mut tried_paths = Vec::new();
    for &tail in &[".rs", "/mod.rs"] {
        let relative_path = search_dir.join(mod_name.to_owned() + tail);
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
    use indoc::indoc;
    use itertools::Itertools;

    use super::*;
    use crate::package::Package;
    use crate::source::SourceFile;

    /// We should not generate mutants that produce the same tokens as the
    /// source.
    #[test]
    fn no_mutants_equivalent_to_source() {
        let code = indoc! { "
            fn always_true() -> bool { true }
        "};
        let source_file = SourceFile {
            code: Arc::new(code.to_owned()),
            package: Arc::new(Package {
                name: "unimportant".to_owned(),
                version: "0.0.1".to_owned(),
                relative_manifest_path: "Cargo.toml".into(),
            }),
            tree_relative_path: Utf8PathBuf::from("src/lib.rs"),
            is_top: true,
        };
        let (mutants, _files) = walk_file(&source_file, &[]).expect("walk_file");
        let mutant_names = mutants.iter().map(|m| m.name(false, false)).collect_vec();
        // It would be good to suggest replacing this with 'false', breaking a key behavior,
        // but bad to replace it with 'true', changing nothing.
        assert_eq!(
            mutant_names,
            ["src/lib.rs: replace always_true -> bool with false"]
        );
    }

    // / As a generic protection against regressions in discovery, the the mutants
    // / generated from `cargo-mutants` own tree against a checked-in list.
    // /
    // / The snapshot will need to be updated when functions are added or removed,
    // / as well as when new mutation patterns are added.
    // /
    // / To stop it being too noisy, we use a custom format with no line numbers.
    #[test]
    fn expected_mutants_for_own_source_tree() {
        let options = Options {
            error_values: vec!["::anyhow::anyhow!(\"mutated!\")".to_owned()],
            show_line_col: false,
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
        insta::assert_snapshot!(list_output);
    }
}
