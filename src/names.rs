use lazy_static::lazy_static;
use regex::Regex;
use rustc_hir::{
    def_id::{DefId, LOCAL_CRATE},
    definitions::DefPathData,
};
use rustc_middle::ty::print::Printer;
use rustc_middle::ty::{print::FmtPrinter, GenericArg, TyCtxt};
use rustc_resolve::Namespace;

lazy_static! {
    static ref RE_NON_LOCAL: [Regex; 1] = [Regex::new(r"(<[^[:alpha:]]*)[^> ]*?::(.*?>)").unwrap()];
    //static ref RE_BOTH: [Regex; 5] = [
    //    Regex::new(r"(for )[^>]*?::(.*?>)").unwrap(),
    //    Regex::new(r"(<impl )[^>]*?::(.*?>)").unwrap(),
    //    Regex::new(r"(<\(dyn )[^>]*?::(.*?>)").unwrap(),
    //    Regex::new(r"(\+ )[^)>]*?::()").unwrap(),
    //    Regex::new(r"(as )[^)>]*?::(.*?>)").unwrap(),
    //];
    static ref RE_LIFETIME: [Regex; 2] = [Regex::new(r"( \+ )?'.+?(, | |(\)|>))").unwrap(), Regex::new(r"<>").unwrap()];
}

/// Custom naming scheme for MIR bodies, adapted from def_path_debug_str() in TyCtxt
pub(crate) fn def_id_name<'tcx>(
    tcx: TyCtxt<'tcx>,
    def_id: DefId,
    substs: &'tcx [GenericArg<'tcx>],
) -> String {
    //let param_env = tcx.param_env(def_id).with_reveal_all_normalized(tcx);
    //let def_id = tcx.normalize_erasing_regions(param_env, def_id);

    let suffix = def_path_str_with_substs_with_no_visible_path(tcx, def_id, substs);

    let mut def_path_str = if !def_id.is_local() && suffix.starts_with("<(dyn") {
        let cstore = tcx.cstore_untracked();

        format!("{}::{}", cstore.crate_name(def_id.krate), suffix)
    } else if def_id.is_local() {
        let crate_name = tcx.crate_name(LOCAL_CRATE);

        format!("{}::{}", crate_name, suffix)
    } else {
        format!("{}", suffix)
    };

    if !def_id.is_local() {
        // In case this is a non-local def_id, the name of the crate is printed in generic types like <foo::Foo>
        // We remove this crate prefix here, because it can lead to discontinuity in the dependency graph (static)
        // and false traces (dynamic)

        for re in RE_NON_LOCAL.iter() {
            def_path_str = re.replace_all(&def_path_str, "${1}${2}").to_string();
        }
    }

    for re in RE_LIFETIME.iter() {
        // Remove lifetime parameters if present
        def_path_str = re.replace_all(&def_path_str, "${3}").to_string();
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
