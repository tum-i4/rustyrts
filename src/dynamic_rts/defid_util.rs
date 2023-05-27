use crate::names::def_id_name;
use log::warn;
use once_cell::sync::OnceCell;
use rustc_hir::def_id::{DefId, LOCAL_CRATE};
use rustc_middle::{
    middle::exported_symbols::{ExportedSymbol, SymbolExportInfo},
    ty::TyCtxt,
};
use rustc_span::def_id::CrateNum;

const RLIB_CRATE_NAME: &str = "rustyrts_dynamic_rlib";

#[cfg(unix)]
const STD_CRATE_NAME: &str = "std";

const PRE_TEST_FN_NAME: &str = "pre_test";
const POST_TEST_FN_NAME: &str = "post_test";
const TRACE_FN_NAME: &str = "trace";

#[cfg(unix)]
const POST_MAIN_FN_NAME: &str = "post_main";

#[cfg(unix)]
const PRE_MAIN_FN_NAME: &str = "pre_main";

#[cfg(unix)]
const EXIT_FN_NAME: &str = "process::exit";

static RLIB_CRATE: OnceCell<Option<CrateNum>> = OnceCell::new();

#[cfg(unix)]
static STD_CRATE: OnceCell<Option<CrateNum>> = OnceCell::new();

#[cfg(unix)]
static PRE_FN_TEST_DEF_ID: OnceCell<Option<DefId>> = OnceCell::new();

#[cfg(unix)]
static PRE_FN_MAIN_DEF_ID: OnceCell<Option<DefId>> = OnceCell::new();

static TRACE_FN_DEF_ID: OnceCell<Option<DefId>> = OnceCell::new();

static POST_FN_TEST_DEF_ID: OnceCell<Option<DefId>> = OnceCell::new();
static POST_FN_MAIN_DEF_ID: OnceCell<Option<DefId>> = OnceCell::new();

#[cfg(unix)]
static EXIT_FN_DEF_ID: OnceCell<Option<DefId>> = OnceCell::new();

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

#[cfg(unix)]
pub(crate) fn get_std_crate(tcx: TyCtxt) -> Option<CrateNum> {
    let std_crate = STD_CRATE.get_or_init(|| get_crate_by_name(tcx, STD_CRATE_NAME));
    *std_crate
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
            let def_path_str = def_id_name(tcx, def_id);
            if def_path_str.ends_with(name) {
                return Some(def_id);
            }
        }
    }

    None
}

pub(crate) fn get_def_id_from_rlib_crate(tcx: TyCtxt, name: &str) -> Option<DefId> {
    let rlib_crate = get_rlib_crate(tcx)?;
    let def_id = get_def_id_exported(tcx, rlib_crate, name);
    if def_id.is_none() {
        warn!(
            "Did not find {} function. Crate {} will not be traced properly.",
            name,
            tcx.crate_name(LOCAL_CRATE)
        );
    }
    def_id
}

pub(crate) fn get_def_id_trace_fn(tcx: TyCtxt) -> Option<DefId> {
    *TRACE_FN_DEF_ID.get_or_init(|| get_def_id_from_rlib_crate(tcx, TRACE_FN_NAME))
}

pub(crate) fn get_def_id_pre_test_fn(tcx: TyCtxt) -> Option<DefId> {
    *PRE_FN_TEST_DEF_ID.get_or_init(|| get_def_id_from_rlib_crate(tcx, PRE_TEST_FN_NAME))
}

#[cfg(unix)]
pub(crate) fn get_def_id_pre_main_fn(tcx: TyCtxt) -> Option<DefId> {
    *PRE_FN_MAIN_DEF_ID.get_or_init(|| get_def_id_from_rlib_crate(tcx, PRE_MAIN_FN_NAME))
}

pub(crate) fn get_def_id_post_test_fn(tcx: TyCtxt) -> Option<DefId> {
    *POST_FN_TEST_DEF_ID.get_or_init(|| get_def_id_from_rlib_crate(tcx, POST_TEST_FN_NAME))
}

#[cfg(unix)]
pub(crate) fn get_def_id_post_main_fn(tcx: TyCtxt) -> Option<DefId> {
    *POST_FN_MAIN_DEF_ID.get_or_init(|| get_def_id_from_rlib_crate(tcx, POST_MAIN_FN_NAME))
}

#[cfg(unix)]
pub(crate) fn get_def_id_exit_fn(tcx: TyCtxt) -> Option<DefId> {
    *EXIT_FN_DEF_ID.get_or_init(|| {
        let std_crate = get_std_crate(tcx)?;
        let def_id = get_def_id_exported(tcx, std_crate, EXIT_FN_NAME);
        if def_id.is_none() {
            warn!(
                "Did not find {} function. Crate {} may not be traced properly.",
                EXIT_FN_NAME,
                tcx.crate_name(LOCAL_CRATE)
            );
        }
        def_id
    })
}
