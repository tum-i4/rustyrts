use rustc_hir::def_id::DefId;
use rustc_middle::{
    middle::exported_symbols::{ExportedSymbol, SymbolExportInfo},
    ty::TyCtxt,
};
use rustc_span::def_id::CrateNum;

use crate::names::def_id_name;

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
    get_crate_by_name(tcx, "rustyrts_dynamic_rlib")
}

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
            if def_path_str == name {
                return Some(def_id);
            }
        }
    }

    None
}
