use std::{
    collections::{BTreeMap, HashMap},
    fs::read_to_string,
    path::PathBuf,
    sync::Mutex,
};

use lazy_static::lazy_static;
use log::{debug, error, trace};
use once_cell::sync::OnceCell;
use regex::Regex;
use rustc_hir::{
    def::DefKind,
    def_id::{DefId, LocalDefId, LOCAL_CRATE},
};
use rustc_middle::ty::TyCtxt;
use rustc_span::Symbol;

use crate::{fs_utils::get_reexports_path, static_rts::callback::PATH_BUF};

pub(crate) static REEXPORTS: OnceCell<
    Mutex<
        HashMap<
            String,
            Option<(
                BTreeMap<String, String>, // Keys are prefixes that need to be replaced
                HashMap<String, String>,  // For Functions
                HashMap<String, String>,  // For Adts
            )>,
        >,
    >,
> = OnceCell::new();

/// Custom naming scheme for MIR bodies, adapted from def_path_debug_str() in TyCtxt
pub(crate) fn def_id_name<'tcx>(tcx: TyCtxt<'tcx>, def_id: DefId) -> String {
    let substs = &[];

    let crate_name = if def_id.is_local() {
        format!(
            "{}[{:04x}]::",
            tcx.crate_name(LOCAL_CRATE),
            tcx.sess.local_stable_crate_id().to_u64() >> 8 * 6
        )
    } else {
        let cstore = tcx.cstore_untracked();

        // 1) We introduce a ! here, to indicate that the element after it has to be deleted
        format!(
            "{}[{:04x}]::!",
            cstore.crate_name(def_id.krate),
            cstore.stable_crate_id(def_id.krate).to_u64() >> 8 * 6
        )
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
        let cstore = tcx.cstore_untracked();

        let crate_name = format!("{}", cstore.crate_name(def_id.krate));
        let crate_id = cstore.stable_crate_id(def_id.krate).to_u64();
        let extended_crate_name = format!("{}[{:04x}]", crate_name, crate_id >> 8 * 6);

        if let Some(mutex) = REEXPORTS.get() {
            let mut reexports = mutex.lock().unwrap();

            // Load exported symbols of this crate if not present
            if !reexports.contains_key(&extended_crate_name) {
                let path =
                    get_reexports_path(PATH_BUF.get().unwrap().clone(), &crate_name, crate_id);
                let content = load_exported_symbols(path);

                if let Some(_) = content {
                    debug!("Loaded reexports of crate {}.", extended_crate_name);
                }

                reexports.insert(extended_crate_name.clone(), content);
            }

            // If this def_id is not local, we check whether it corresponds to a name reexported by another crate
            // If this is the case, we replace the deviating part of the name by its counterpart in the other crate
            if let Some(maybe_maps) = reexports.get(&extended_crate_name) {
                if let Some((prefix_map, fn_map, adt_map)) = maybe_maps {
                    let kind = tcx.def_kind(def_id);

                    if let DefKind::Fn = kind {
                        if let Some(replacement) = fn_map.get(&def_path_str) {
                            trace!("Found Fn {} - replaced by {}", def_path_str, replacement);
                            return replacement.clone();
                        }
                    }

                    if let DefKind::Struct | DefKind::Enum | DefKind::Trait = kind {
                        if let Some(replacement) = adt_map.get(&def_path_str) {
                            trace!("Found Adt {} - replaced by {}", def_path_str, replacement);
                            return replacement.clone();
                        }
                    }

                    // If we did not return in the two branches above, we check whether we need to replace
                    //the prefix of the path of some other module
                    if let Some((maybe_prefix, replacement)) =
                        prefix_map.range(..def_path_str.clone()).next_back()
                    {
                        if def_path_str.starts_with(maybe_prefix) {
                            trace!(
                                "Found Mod {} - prefix {} replaced by {}",
                                def_path_str,
                                maybe_prefix,
                                replacement
                            );

                            return def_path_str.replace(maybe_prefix, &replacement);
                        }
                    }
                }
            }
        }
    }

    def_path_str
}

fn load_exported_symbols(
    path: PathBuf,
) -> Option<(
    BTreeMap<String, String>,
    HashMap<String, String>,
    HashMap<String, String>,
)> {
    let mut prefix_map = BTreeMap::new();
    let mut fn_map = HashMap::new();
    let mut adt_map = HashMap::new();

    read_to_string(path.as_path())
        .ok()?
        .split("\n")
        .map(|s| s.to_string())
        .filter(|line| !line.is_empty())
        .map(|line| {
            line.split_once(" | ")
                .map(|(s1, s2)| (s1.to_string(), s2.to_string()))
                .unwrap()
        })
        .into_iter()
        .for_each(|(s1, s2)| {
            if s1.ends_with("::") {
                if let Some(x) = prefix_map.insert(s1.clone(), s2.clone()) {
                    error!("{} overwritten by {}", x, s2.clone());
                }
            } else {
                if s1.ends_with("!adt") {
                    // If this is an Adt, insert in both adt_map and prefix_map
                    // Because either the Adt may be used (directly) or its associated functions (via prefix)
                    let s1 = s1.strip_suffix("!adt").unwrap().to_string();

                    if let Some(x) = prefix_map.insert(s1.clone(), s2.clone()) {
                        error!("{} overwritten by {}", x, s2.clone());
                    }
                    if let Some(x) = adt_map.insert(s1.clone(), s2.clone()) {
                        error!("{} overwritten by {}", x, s2.clone());
                    }
                } else {
                    if let Some(x) = fn_map.insert(s1.clone(), s2.clone()) {
                        error!("{} overwritten by {}", x, s2.clone());
                    }
                }
            }
        });
    Some((prefix_map, fn_map, adt_map))
}

pub(crate) fn exported_name<'tcx>(
    tcx: TyCtxt<'tcx>,
    mod_def_id: LocalDefId,
    symbol: Symbol,
) -> String {
    let mut mod_name = def_id_name(tcx, mod_def_id.to_def_id());

    if !mod_name.ends_with("::") {
        mod_name += "::";
    }

    let def_path_str = format!("{}{}", mod_name, symbol.as_str());
    def_path_str
}
