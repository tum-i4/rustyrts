use lazy_static::lazy_static;
use regex::Regex;
use rustc_hir::{def_id::DefId, definitions::DefPathData, def::Namespace};
use rustc_middle::ty::{print::FmtPrinter, TyCtxt};
use rustc_middle::ty::{print::Printer, List};

lazy_static! {
    static ref RE_LIFETIME: [Regex; 2] = [
        Regex::new(r"( \+ )?'.+?(, | |(\)|>))").unwrap(),
        Regex::new(r"(::)?<>").unwrap()
    ];
}

/// Custom naming scheme for MIR bodies, adapted from def_path_debug_str() in TyCtxt
pub(crate) fn def_id_name<'tcx>(
    tcx: TyCtxt<'tcx>,
    def_id: DefId,
    add_crate_id: bool,
    trimmed: bool,
) -> String {
    assert!(trimmed | def_id.is_local());

    let crate_id = if add_crate_id {
        if def_id.is_local() {
            format!(
                "[{:04x}]::",
                tcx.sess.local_stable_crate_id().as_u64() >> (8 * 6)
            )
        } else {
            let cstore = tcx.cstore_untracked();

            format!(
                "[{:04x}]::",
                cstore.stable_crate_id(def_id.krate).as_u64() >> (8 * 6)
            )
        }
    } else {
        "".to_string()
    };

    let suffix = def_path_str_with_substs_with_no_visible_path(tcx, def_id, trimmed);

    let crate_name = {
        let name = format!("{}::", tcx.crate_name(def_id.krate));
        if def_id.is_local() || !suffix.starts_with(&name) {
            name
        } else {
            "".to_string()
        }
    };

    let mut def_path_str = format!("{}{}{}", crate_id, crate_name, suffix);

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
    trimmed: bool,
) -> String {
    let ns = guess_def_namespace(tcx, def_id);

    if trimmed {
        rustc_middle::ty::print::with_forced_trimmed_paths!(
            rustc_middle::ty::print::with_no_visible_paths!(
                FmtPrinter::new(tcx, ns).print_def_path(def_id, List::empty())
            )
        )
    } else {
        rustc_middle::ty::print::with_no_visible_paths!(
            FmtPrinter::new(tcx, ns).print_def_path(def_id, List::empty())
        )
    }
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
