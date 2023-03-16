use crate::names::def_id_name;
use once_cell::sync::OnceCell;
use rustc_hir::def_id::{DefId, LOCAL_CRATE};
use rustc_middle::{
    middle::exported_symbols::{ExportedSymbol, SymbolExportInfo},
    ty::TyCtxt,
};
use rustc_span::def_id::CrateNum;

const RLIB_CRATE_NAME: &str = "rustyrts_dynamic_rlib";

const PRE_FN_NAME: &str = "pre_processing";
const POST_FN_NAME: &str = "post_processing";
const TRACE_FN_NAME: &str = "trace";

static RLIB_CRATE: OnceCell<Option<CrateNum>> = OnceCell::new();

static PRE_FN_DEF_ID: OnceCell<Option<DefId>> = OnceCell::new();
static TRACE_FN_DEF_ID: OnceCell<Option<DefId>> = OnceCell::new();
static POST_FN_DEF_ID: OnceCell<Option<DefId>> = OnceCell::new();

pub(crate) fn get_crate_by_name(tcx: TyCtxt, name: &str) -> Option<CrateNum> {
    let crates = tcx.crates(());

    for krate in crates {
        if tcx.crate_name(*krate).as_str() == name {
            return Some(*krate);
        }
    }
    None
}

pub(crate) fn get_rlib_crate(tcx: TyCtxt) -> Option<CrateNum> {
    let rlib_crate = RLIB_CRATE.get_or_init(|| get_crate_by_name(tcx, RLIB_CRATE_NAME));
    *rlib_crate
}

#[allow(dead_code)]
pub(crate) fn for_each_exported_symbols<F>(tcx: TyCtxt, krate: CrateNum, func: F)
where
    F: Fn(&(ExportedSymbol<'_>, SymbolExportInfo)),
{
    let symbols = tcx.exported_symbols(krate);

    for symbol in symbols {
        func(symbol);
    }
}

pub(crate) fn get_def_id_exported(tcx: TyCtxt, krate: CrateNum, name: &str) -> Option<DefId> {
    let symbols = tcx.exported_symbols(krate);

    for symbol in symbols {
        let maybe_def_id = match symbol.0 {
            ExportedSymbol::NonGeneric(def_id) => Some(def_id),
            ExportedSymbol::Generic(def_id, _subst) => Some(def_id),
            _ => None,
        };

        if let Some(def_id) = maybe_def_id {
            let def_path_str = def_id_name(tcx, def_id).expect_one();
            if def_path_str == name {
                return Some(def_id);
            }
        }
    }

    None
}

pub(crate) fn get_def_id_from_rlib_crate(tcx: TyCtxt, name: &str) -> Option<DefId> {
    let rlib_crate = get_rlib_crate(tcx)?;
    let name = format!("{}::{}", tcx.crate_name(rlib_crate), name);
    let def_id = get_def_id_exported(tcx, rlib_crate, name.as_str());
    if def_id.is_none() {
        eprintln!(
            "Did not find {} function. Crate {} will not be traced properly.",
            name,
            tcx.crate_name(LOCAL_CRATE)
        );
    }
    def_id
}

pub(crate) fn get_def_id_pre_fn(tcx: TyCtxt) -> Option<DefId> {
    *PRE_FN_DEF_ID.get_or_init(|| get_def_id_from_rlib_crate(tcx, PRE_FN_NAME))
}

pub(crate) fn get_def_id_trace_fn(tcx: TyCtxt) -> Option<DefId> {
    *TRACE_FN_DEF_ID.get_or_init(|| get_def_id_from_rlib_crate(tcx, TRACE_FN_NAME))
}

pub(crate) fn get_def_id_post_fn(tcx: TyCtxt) -> Option<DefId> {
    *POST_FN_DEF_ID.get_or_init(|| get_def_id_from_rlib_crate(tcx, POST_FN_NAME))
}
