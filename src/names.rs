use std::collections::{BTreeMap, HashMap, HashSet};

use itertools::Itertools;
use lazy_static::lazy_static;
use once_cell::sync::OnceCell;
use regex::Regex;
use rustc_hir::{
    def::DefKind,
    def_id::{DefId, LOCAL_CRATE},
};
use rustc_middle::ty::TyCtxt;
use rustc_span::Symbol;
use std::fmt::Debug;
use std::hash::Hash;

pub(crate) static REEXPORTS: OnceCell<(
    BTreeMap<String, HashSet<String>>, // Keys are prefixes that need to be replaced
    HashMap<String, HashSet<String>>,  // For Functions
    HashMap<String, HashSet<String>>,  // For Adts
)> = OnceCell::new();

#[derive(Debug)]
pub(crate) enum OneOrMore<T> {
    One(T),
    More(HashSet<T>),
}

impl<T: Debug> OneOrMore<T> {
    pub fn expect_one(self) -> T {
        match self {
            OneOrMore::One(e) => e,
            OneOrMore::More(s) => {
                if s.len() == 1 {
                    s.into_iter().next().unwrap()
                } else {
                    panic!("Expected only one item: {:?}", s)
                }
            }
        }
    }
}

impl<T> IntoIterator for OneOrMore<T>
where
    T: Hash + Eq,
{
    type Item = T;

    type IntoIter = <HashSet<T> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        match self {
            OneOrMore::One(e) => {
                let mut set = HashSet::new();
                set.insert(e);
                set.into_iter()
            }
            OneOrMore::More(h) => h.into_iter(),
        }
    }
}

/// Custom naming scheme for MIR bodies, adapted from def_path_debug_str() in TyCtxt
pub(crate) fn def_id_name<'tcx>(tcx: TyCtxt<'tcx>, def_id: DefId) -> OneOrMore<String> {
    // TODO: check if this is problematic:
    let substs = &[]; //  List::identity_for_item(tcx, def_id);

    let crate_name = if def_id.is_local() {
        format!("{}::", tcx.crate_name(LOCAL_CRATE))
    } else {
        let cstore = tcx.cstore_untracked();

        // 1) We introduce a ! here, to indicate that the element after it has to be deleted
        format!("{}::!", cstore.crate_name(def_id.krate))
    };

    let mut def_path_str = format!(
        "{}{}",
        crate_name,
        tcx.def_path_str_with_substs(def_id, substs)
    );

    // This is a hack
    //      We are removing the crate prefix in the type that is casted to
    //      This prefix is present if the type is from a non-local crate
    //      We do not want to keep it
    lazy_static! {
        static ref REGEX_CRATE_PREFIX: Regex = Regex::new(r"(<.* as )(.*::)(.*>)").unwrap();
    }
    def_path_str = REGEX_CRATE_PREFIX
        .replace_all(&def_path_str, "$1$3")
        .to_string();

    // This is a hack
    // See 1) above
    // If this is a non-local def_id:
    //      We are removing the part of the path that corresponds to the alias name of the extern crate
    //      In this extern crate itself, this part of the path is not present

    lazy_static! {
        static ref REGEX_LOCAL_ALIAS_1: Regex = Regex::new(r"(!)<").unwrap();
    }
    def_path_str = REGEX_LOCAL_ALIAS_1
        .replace_all(&def_path_str, "<")
        .to_string();

    lazy_static! {
        static ref REGEX_LOCAL_ALIAS_2: Regex = Regex::new(r"(![^:]*?::)").unwrap();
    }
    def_path_str = REGEX_LOCAL_ALIAS_2
        .replace_all(&def_path_str, "")
        .to_string();

    // Occasionally, there is a newline which we do not want to keep
    def_path_str = def_path_str.replace("\n", "");

    if !def_id.is_local() {
        // If this def_id is not local, we check whether it corresponds to a name reexported by another crate
        // If this is the case, we replace the deviating part of the name by its counterpart in the other crate
        if let Some((prefix_map, fn_map, adt_map)) = REEXPORTS.get() {
            let kind = tcx.def_kind(def_id);

            if let DefKind::Fn = kind {
                if let Some(replacements) = fn_map.get(&def_path_str) {
                    //println!("Found Fn {} - replaced by {}", def_path_str, replacement);

                    if replacements.len() > 1 {
                        println!("Found multiple symbols that are exported as {}: {} - maybe multiple versions of the corresponding crate have been compiled",
                        def_path_str,
                            replacements.iter().join(", ")
                        );
                    }

                    return OneOrMore::More(replacements.clone());
                }
            }

            if let DefKind::Struct | DefKind::Enum | DefKind::Trait = kind {
                if let Some(replacements) = adt_map.get(&def_path_str) {
                    //println!("Found Adt {} - replaced by {}", def_path_str, replacement);

                    if replacements.len() > 1 {
                        println!("Found multiple symbols that are exported as {}: {} - maybe multiple versions of the corresponding crate have been compiled",
                        def_path_str,
                            replacements.iter().join(", ")
                        );
                    }
                    return OneOrMore::More(replacements.clone());
                }
            }

            // If we did not return in the two branches above, we check whether we need to replace
            //the prefix of the path of some other module
            if let Some((maybe_prefix, replacements)) =
                prefix_map.range(..def_path_str.clone()).next_back()
            {
                if def_path_str.starts_with(maybe_prefix) {
                    //println!(
                    //    "Found Mod {} - prefix {} replaced by {}",
                    //    def_path_str, predecessor, replacement
                    //);

                    if replacements.len() > 1 {
                        println!("Found multiple symbols that are exported as {}: {} - maybe multiple versions of the corresponding crate have been compiled",
                            maybe_prefix,
                            replacements.iter().join(", ")
                        );
                    }

                    let mut ret = HashSet::new();
                    for replacement in replacements {
                        ret.insert(def_path_str.replace(maybe_prefix, &replacement));
                    }
                    return OneOrMore::More(ret);
                }
            }
        }
    }

    OneOrMore::One(def_path_str)
}

pub(crate) fn exported_name<'tcx>(tcx: TyCtxt<'tcx>, symbol: Symbol) -> String {
    let crate_name = format!("{}::", tcx.crate_name(LOCAL_CRATE));

    let def_path_str = format!("{}{}", crate_name, symbol.as_str());
    def_path_str
}
