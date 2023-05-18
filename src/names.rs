use lazy_static::lazy_static;
use regex::Regex;
use rustc_hir::{
    def_id::{DefId, LOCAL_CRATE},
    definitions::DefPathData,
};
use rustc_middle::ty::print::Printer;
use rustc_middle::ty::{print::FmtPrinter, GenericArg, TyCtxt};
use rustc_resolve::Namespace;

/// Custom naming scheme for MIR bodies, adapted from def_path_debug_str() in TyCtxt
pub(crate) fn def_id_name<'tcx>(tcx: TyCtxt<'tcx>, def_id: DefId) -> String {
    let substs = &[];

    let crate_path = if def_id.is_local() {
        let crate_name = tcx.crate_name(LOCAL_CRATE);

        format!(
            "[{:04x}]::{}",
            tcx.sess.local_stable_crate_id().to_u64() >> 8 * 6,
            crate_name
        )
    } else {
        let cstore = tcx.cstore_untracked();

        format!(
            "[{:04x}]",
            cstore.stable_crate_id(def_id.krate).to_u64() >> 8 * 6
        )
    };

    let mut def_path_str = format!(
        "{}::{}",
        crate_path,
        def_path_str_with_substs_with_no_visible_path(tcx, def_id, substs)
    );

    if !def_id.is_local() {
        // In case this is a non-local def_id, the name of the crate is printed in generic types like <foo::Foo>
        // We remove this crate prefix here, because it can lead to discontinuity in the dependency graph (static)
        // and false traces (dynamic)

        lazy_static! {
            static ref RE_GENERICS_IMPL: Regex = Regex::new(r"<impl ([^>]*?)::").unwrap();
            static ref RE_GENERICS_FOR: Regex = Regex::new(r"for ([^>]*?)::").unwrap();
            static ref RE_GENERICS: Regex = Regex::new(r"<([^> ]*?)::").unwrap();
        }

        def_path_str = RE_GENERICS_IMPL
            .replace_all(&def_path_str, "<impl ")
            .to_string();
        def_path_str = RE_GENERICS_FOR
            .replace_all(&def_path_str, "for ")
            .to_string();
        def_path_str = RE_GENERICS.replace_all(&def_path_str, "<").to_string();
    }

    // Occasionally, there is a newline which we do not want to keep
    def_path_str = def_path_str.replace("\n", "");

    def_path_str
}

pub fn def_path_str_with_substs_with_no_visible_path<'t>(
    tcx: TyCtxt<'t>,
    def_id: DefId,
    substs: &'t [GenericArg<'t>],
) -> String {
    let ns = guess_def_namespace(tcx, def_id);

    rustc_middle::ty::print::with_no_visible_paths!(
        FmtPrinter::new(tcx, ns).print_def_path(def_id, substs)
    )
    .unwrap()
    .into_buffer()

    //lazy_static! {
    //    static ref RE_GENERICS: Regex = Regex::new(r"<.*>::").unwrap();
    //}
    //
    //RE_GENERICS.replace_all(&result, "").to_string()
}

// Source: https://doc.rust-lang.org/stable/nightly-rustc/src/rustc_middle/ty/print/pretty.rs.html#1766
// HACK(eddyb) get rid of `def_path_str` and/or pass `Namespace` explicitly always
// (but also some things just print a `DefId` generally so maybe we need this?)
fn guess_def_namespace(tcx: TyCtxt<'_>, def_id: DefId) -> Namespace {
    match tcx.def_key(def_id).disambiguated_data.data {
        DefPathData::TypeNs(..) | DefPathData::CrateRoot | DefPathData::ImplTrait => {
            Namespace::TypeNS
        }

        DefPathData::ValueNs(..)
        | DefPathData::AnonConst
        | DefPathData::ClosureExpr
        | DefPathData::Ctor => Namespace::ValueNS,

        DefPathData::MacroNs(..) => Namespace::MacroNS,

        _ => Namespace::TypeNS,
    }
}
