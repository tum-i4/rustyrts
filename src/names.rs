use lazy_static::lazy_static;
use regex::Regex;
use rustc_data_structures::stable_hasher::ToStableHashKey;
use rustc_hir::{def::Namespace, def_id::DefId, definitions::DefPathData};
use rustc_middle::ty::{
    print::FmtPrinter, AliasTy, Binder, FnSig, GenericArgs, ParamTy, Ty, TyCtxt, TypeAndMut,
};
use rustc_middle::ty::{print::Printer, List};
use rustc_span::{def_id::LOCAL_CRATE, Symbol};
use rustc_type_ir::TyKind;

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
    mono_def_id_name(tcx, def_id, List::empty(), add_crate_id, trimmed)
}

/// Custom naming scheme for MIR bodies, adapted from def_path_debug_str() in TyCtxt
pub(crate) fn mono_def_id_name<'tcx>(
    tcx: TyCtxt<'tcx>,
    def_id: DefId,
    substs: &'tcx GenericArgs<'tcx>,
    add_crate_id: bool,
    trimmed: bool,
) -> String {
    assert!(trimmed | def_id.is_local());

    let substs = filter_generic_args(tcx, substs, 1);

    let crate_id = if add_crate_id {
        if def_id.is_local() {
            format!(
                "[{:04x}]::",
                tcx.stable_crate_id(LOCAL_CRATE).as_u64() >> (8 * 6)
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

    let suffix = def_path_str_with_substs_with_no_visible_path(tcx, def_id, substs, trimmed);

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
    substs: &'t GenericArgs<'t>,
    trimmed: bool,
) -> String {
    let ns = guess_def_namespace(tcx, def_id);

    let mut printer = FmtPrinter::new(tcx, ns);

    if trimmed {
        rustc_middle::ty::print::with_forced_trimmed_paths!(
            rustc_middle::ty::print::with_no_visible_paths!(printer.print_def_path(def_id, substs))
        )
    } else {
        rustc_middle::ty::print::with_no_visible_paths!(printer.print_def_path(def_id, substs))
    }
    .unwrap();

    printer.into_buffer()
}

// Source: https://doc.rust-lang.org/nightly/nightly-rustc/src/rustc_middle/ty/print/pretty.rs.html#1803
// HACK(eddyb) get rid of `def_path_str` and/or pass `Namespace` explicitly always
// (but also some things just print a `DefId` generally so maybe we need this?)
fn guess_def_namespace(tcx: TyCtxt<'_>, def_id: DefId) -> Namespace {
    match tcx.def_key(def_id).disambiguated_data.data {
        DefPathData::TypeNs(..) | DefPathData::CrateRoot | DefPathData::OpaqueTy => {
            Namespace::TypeNS
        }

        DefPathData::ValueNs(..)
        | DefPathData::AnonConst
        | DefPathData::Closure
        | DefPathData::Ctor => Namespace::ValueNS,

        DefPathData::MacroNs(..) => Namespace::MacroNS,

        _ => Namespace::TypeNS,
    }
}

#[allow(dead_code)]
fn filter_generic_args<'tcx>(
    tcx: TyCtxt<'tcx>,
    args: &'tcx GenericArgs<'tcx>,
    depth: usize,
) -> &'tcx GenericArgs<'tcx> {
    let mut new_args = Vec::new();

    for arg in args.into_iter() {
        let new_arg = if let Some(ty) = arg.as_type() {
            filter_ty(tcx, ty, depth).into()
        } else {
            arg
        };
        new_args.push(new_arg);
    }

    tcx.mk_args(&new_args)
}

fn get_placeholder<'tcx>(tcx: TyCtxt<'tcx>, content: &str) -> Ty<'tcx> {
    let param_ty = ParamTy::new(0, Symbol::intern(content));
    tcx.mk_ty_from_kind(TyKind::Param(param_ty))
}

const MAX_MONOMORPHIZATION_DEPTH: usize = 4;

fn filter_ty<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>, depth: usize) -> Ty<'tcx> {
    if depth > MAX_MONOMORPHIZATION_DEPTH {
        return get_placeholder(tcx, "..");
    }

    let kind = match ty.kind() {
        TyKind::Bool => {
            return ty;
        }
        TyKind::Char => {
            return ty;
        }
        TyKind::Int(_int_ty) => {
            return ty;
        }
        TyKind::Uint(_uint_ty) => {
            return ty;
        }
        TyKind::Float(_float_ty) => {
            return ty;
        }
        TyKind::Adt(def, args) => TyKind::Adt(*def, filter_generic_args(tcx, args, depth + 1)),
        TyKind::Foreign(_def_id) => {
            return ty;
        }
        TyKind::Str => {
            return ty;
        }
        TyKind::Array(ty, const_) => TyKind::Array(filter_ty(tcx, *ty, depth + 1), *const_),
        TyKind::Slice(ty) => TyKind::Slice(filter_ty(tcx, *ty, depth + 1)),
        TyKind::RawPtr(TypeAndMut { ty, mutbl }) => TyKind::RawPtr(TypeAndMut {
            ty: filter_ty(tcx, *ty, depth + 1),
            mutbl: *mutbl,
        }),
        TyKind::Ref(region, ty, mutbl) => {
            TyKind::Ref(*region, filter_ty(tcx, *ty, depth + 1), *mutbl)
        }
        TyKind::FnDef(def_id, args) => {
            TyKind::FnDef(*def_id, filter_generic_args(tcx, *args, depth + 1))
        }
        TyKind::FnPtr(poly_fn_sig) => {
            let bound = poly_fn_sig.bound_vars();
            let sig = poly_fn_sig.skip_binder();
            let list = filter_tys(tcx, sig.inputs_and_output, depth + 1);
            let new_inputs_and_outputs = tcx.mk_type_list(&list);
            let new_sig = FnSig {
                inputs_and_output: new_inputs_and_outputs,
                c_variadic: sig.c_variadic,
                unsafety: sig.unsafety,
                abi: sig.abi,
            };
            TyKind::FnPtr(Binder::bind_with_vars(new_sig, bound))
        }
        TyKind::Dynamic(_predicates, _region, _kind) => {
            return ty;
        } // TODO
        TyKind::Closure(def_id, _args) => {
            // TyKind::Closure(*def_id, filter_generic_args(tcx, *def_id, *args))
            let name = format!(
                "Fn#{}",
                tcx.with_stable_hashing_context(|hasher| def_id.to_stable_hash_key(&hasher))
                    .0
            );
            return get_placeholder(tcx, &name);
        }
        TyKind::Coroutine(def_id, _args, _mvblt) => {
            // TyKind::Coroutine(*def_id, filter_generic_args(tcx, *def_id, *args), *mvblt)
            let name = format!(
                "Fn#{}",
                tcx.with_stable_hashing_context(|hasher| def_id.to_stable_hash_key(&hasher))
                    .0
            );
            return get_placeholder(tcx, &name);
        }
        TyKind::CoroutineWitness(def_id, args) => {
            TyKind::CoroutineWitness(*def_id, filter_generic_args(tcx, *args, depth + 1))
        }
        TyKind::Never => {
            return ty;
        }
        TyKind::Tuple(tys) => {
            let list = filter_tys(tcx, tys, depth + 1);
            TyKind::Tuple(tcx.mk_type_list(&list))
        }
        TyKind::Alias(kind, alias_ty) => {
            let new_alias_ty = AliasTy::new(
                tcx,
                alias_ty.def_id,
                filter_generic_args(tcx, alias_ty.args, depth + 1),
            );
            TyKind::Alias(*kind, new_alias_ty)
        }
        TyKind::Param(_param_ty) => {
            return ty;
        }
        TyKind::Bound(_idx, _bound_ty) => {
            return ty;
        }
        TyKind::Placeholder(_placeholder_ty) => {
            return ty;
        }
        TyKind::Infer(_infer_ty) => {
            return ty;
        }
        TyKind::Error(_error_ty) => {
            return ty;
        }
    };
    tcx.mk_ty_from_kind(kind)
}

fn filter_tys<'tcx>(tcx: TyCtxt<'tcx>, tys: &'tcx List<Ty<'tcx>>, depth: usize) -> Vec<Ty<'tcx>> {
    tys.into_iter()
        .map(|ty| filter_ty(tcx, ty, depth))
        .collect()
}
