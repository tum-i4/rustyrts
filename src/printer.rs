use crate::rustc_middle::ty::print::Printer;
use crate::rustc_middle::ty::DefIdTree;
use rustc_hir::def_id::DefId;
use rustc_hir::definitions::DefPathData;
use rustc_middle::ty::print::{characteristic_def_id_of_type, FmtPrinter};
use rustc_middle::ty::{self, GenericArg, Ty, TyCtxt};
use rustc_resolve::Namespace;

pub fn custom_def_path_str_with_substs<'t>(
    tcx: TyCtxt<'t>,
    def_id: DefId,
    substs: &'t [GenericArg<'t>],
) -> String {
    let ns = guess_def_namespace(tcx, def_id);
    let ret = FmtPrinter::new(tcx, ns)
        .custom_default_print_def_path(def_id, substs)
        .unwrap()
        .into_buffer();

    if let Some(without_prefix) = ret.strip_prefix("::") {
        without_prefix.to_string()
    } else {
        ret
    }
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

pub trait CustomDefPathPrinter<'tcx> {
    type Error;
    type Path;

    fn custom_default_print_def_path(
        self,
        def_id: DefId,
        substs: &'tcx [GenericArg<'tcx>],
    ) -> Result<Self::Path, Self::Error>;

    fn custom_print_impl_path(
        self,
        impl_def_id: DefId,
        substs: &'tcx [GenericArg<'tcx>],
        self_ty: Ty<'tcx>,
        trait_ref: Option<ty::TraitRef<'tcx>>,
    ) -> Result<Self::Path, Self::Error>;
}

impl<'a, 'tcx> CustomDefPathPrinter<'tcx> for FmtPrinter<'a, 'tcx> {
    type Error = core::fmt::Error;
    type Path = Self;

    // Adapted from default_print_def_path
    // Source: https://doc.rust-lang.org/stable/nightly-rustc/src/rustc_middle/ty/print/mod.rs.html#101
    fn custom_default_print_def_path(
        self,
        def_id: DefId,
        substs: &'tcx [GenericArg<'tcx>],
    ) -> Result<Self::Path, Self::Error> {
        let key = self.tcx().def_key(def_id);
        //debug!(?key);

        match key.disambiguated_data.data {
            DefPathData::CrateRoot => {
                assert!(key.parent.is_none());

                // !!! Changed: This is really creating trouble and the actual origin of all those changes
                //self.path_crate(def_id.krate)

                Ok(self)
            }

            DefPathData::Impl => {
                //let generics = self.tcx().generics_of(def_id);
                let self_ty = self.tcx().type_of(def_id);
                let impl_trait_ref = self.tcx().impl_trait_ref(def_id);
                //let (self_ty, impl_trait_ref) = if substs.len() >= generics.count() {
                //    (
                //        self_ty.subst(self.tcx(), substs),
                //        impl_trait_ref.map(|i| i.subst(self.tcx(), substs)),
                //    )
                //} else {
                //    (self_ty.0, impl_trait_ref.map(|i| i.0))
                //};
                // !!! Changed: Some fields that are used above, are private, but somehow it works like this
                self.custom_print_impl_path(def_id, substs, self_ty, impl_trait_ref.map(|i| i.0))
            }

            _ => {
                let parent_def_id = DefId {
                    index: key.parent.unwrap(),
                    ..def_id
                };

                let mut parent_substs = substs;
                let mut trait_qualify_parent = false;
                if !substs.is_empty() {
                    let generics = self.tcx().generics_of(def_id);
                    parent_substs = &substs[..generics.parent_count.min(substs.len())];

                    match key.disambiguated_data.data {
                        // Closures' own generics are only captures, don't print them.
                        DefPathData::ClosureExpr => {}
                        // This covers both `DefKind::AnonConst` and `DefKind::InlineConst`.
                        // Anon consts doesn't have their own generics, and inline consts' own
                        // generics are their inferred types, so don't print them.
                        DefPathData::AnonConst => {}

                        // If we have any generic arguments to print, we do that
                        // on top of the same path, but without its own generics.
                        _ => {
                            if !generics.params.is_empty() && substs.len() >= generics.count() {
                                let args = generics.own_substs_no_defaults(self.tcx(), substs);
                                return self.path_generic_args(
                                    |cx| cx.print_def_path(def_id, parent_substs),
                                    args,
                                );
                            }
                        }
                    }

                    // FIXME(eddyb) try to move this into the parent's printing
                    // logic, instead of doing it when printing the child.
                    trait_qualify_parent = generics.has_self
                        && generics.parent == Some(parent_def_id)
                        && parent_substs.len() == generics.parent_count
                        && self.tcx().generics_of(parent_def_id).parent_count == 0;
                }

                self.path_append(
                    |cx: Self| {
                        if trait_qualify_parent {
                            let trait_ref = cx
                                .tcx()
                                .mk_trait_ref(parent_def_id, parent_substs.iter().copied());
                            cx.path_qualified(trait_ref.self_ty(), Some(trait_ref))
                        } else {
                            // !!! Changed: We need to delegate to our custom fn here
                            cx.custom_default_print_def_path(parent_def_id, parent_substs)
                        }
                    },
                    &key.disambiguated_data,
                )
            }
        }
    }

    // Adapted from default_print_impl_path
    // Source: https://doc.rust-lang.org/stable/nightly-rustc/src/rustc_middle/ty/print/mod.rs.html#185
    fn custom_print_impl_path(
        self,
        impl_def_id: DefId,
        _substs: &'tcx [GenericArg<'tcx>],
        self_ty: Ty<'tcx>,
        impl_trait_ref: Option<ty::TraitRef<'tcx>>,
    ) -> Result<Self::Path, Self::Error> {
        //debug!(
        //    "default_print_impl_path: impl_def_id={:?}, self_ty={}, impl_trait_ref={:?}",
        //    impl_def_id, self_ty, impl_trait_ref
        //);

        let key = self.tcx().def_key(impl_def_id);
        let parent_def_id = DefId {
            index: key.parent.unwrap(),
            ..impl_def_id
        };

        // Decide whether to print the parent path for the impl.
        // Logically, since impls are global, it's never needed, but
        // users may find it useful. Currently, we omit the parent if
        // the impl is either in the same module as the self-type or
        // as the trait.
        let in_self_mod = match characteristic_def_id_of_type(self_ty) {
            None => false,
            Some(ty_def_id) => self.tcx().parent(ty_def_id) == parent_def_id,
        };
        let in_trait_mod = match impl_trait_ref {
            None => false,
            Some(trait_ref) => self.tcx().parent(trait_ref.def_id) == parent_def_id,
        };

        if !in_self_mod && !in_trait_mod {
            // If the impl is not co-located with either self-type or
            // trait-type, then fallback to a format that identifies
            // the module more clearly.
            self.path_append_impl(
                // !!! Changed: We need to delegate to our custom fn here
                |cx| cx.custom_default_print_def_path(parent_def_id, &[]),
                &key.disambiguated_data,
                self_ty,
                impl_trait_ref,
            )
        } else {
            // Otherwise, try to give a good form that would be valid language
            // syntax. Preferably using associated item notation.
            self.path_qualified(self_ty, impl_trait_ref)
        }
    }
}
