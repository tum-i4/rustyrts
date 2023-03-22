use itertools::Itertools;
use log::{debug, trace};
use rustc_hir::{
    def::{DefKind, Res},
    def_id::LOCAL_CRATE,
    ConstContext,
};
use rustc_middle::ty::{TyCtxt, Visibility};
use std::fmt::Display;
use std::hash::Hash;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs::{read, read_dir, DirEntry},
    path::PathBuf,
};

use crate::{
    checksums::{get_checksum, Checksums},
    constants::ENDING_REEXPORTS,
    fs_utils::{
        get_changes_path, get_checksums_path, get_reexports_path, get_test_path,
        read_lines_filter_map, write_to_file,
    },
    names::{def_id_name, exported_name, REEXPORTS},
};

const EXCLUDED_CRATES: &[&str] = &["build_script_build", "build_script_main"];

pub(crate) const TEST_MARKER: &str = "rustc_test_marker";

pub(crate) fn excluded<'tcx>(tcx: TyCtxt<'tcx>) -> bool {
    let local_crate_name = tcx.crate_name(LOCAL_CRATE);
    EXCLUDED_CRATES
        .iter()
        .any(|krate| *krate == local_crate_name.as_str())
}

pub(crate) fn prepare_analysis(path_buf: PathBuf) {
    REEXPORTS.get_or_init(|| {
        let files: Vec<DirEntry> = read_dir(path_buf.as_path())
            .unwrap()
            .map(|maybe_path| maybe_path.unwrap())
            .collect();

        let mut prefix_map = BTreeMap::new();
        let mut fn_map = HashMap::new();
        let mut adt_map = HashMap::new();

        read_lines_filter_map(
            &files,
            ENDING_REEXPORTS,
            |line| !line.is_empty(),
            |line| {
                line.split_once(" | ")
                    .map(|(s1, s2)| (s1.to_string(), s2.to_string()))
                    .unwrap()
            },
        )
        .into_iter()
        .for_each(|(s1, s2)| {
            if s1.ends_with("::") {
                insert_btreemap(&mut prefix_map, s1.clone(), s2.clone());
            } else {
                if s1.ends_with("!adt") {
                    // If this is an Adt, insert in both adt_map and prefix_map
                    // Because either the Adt may be used (directly) or its associated functions (via prefix)
                    let s1 = s1.strip_suffix("!adt").unwrap().to_string();

                    insert_btreemap(&mut prefix_map, s1.clone(), s2.clone());
                    insert_hashmap(&mut adt_map, s1.clone(), s2.clone());
                } else {
                    insert_hashmap(&mut fn_map, s1.clone(), s2.clone());
                }
            }
        });
        (prefix_map, fn_map, adt_map)
    });
}

fn insert_btreemap<K: Ord + Clone + Display, V: Clone + Hash + Eq + Display>(
    map: &mut BTreeMap<K, HashSet<V>>,
    key: K,
    value: V,
) {
    if let None = map.get(&key) {
        map.insert(key.clone(), HashSet::new()).unwrap_or_default();
    }
    map.get_mut(&key).unwrap().insert(value);
}

fn insert_hashmap<K: Hash + Eq + Clone, V: Hash + Eq>(
    map: &mut HashMap<K, HashSet<V>>,
    key: K,
    value: V,
) {
    if let None = map.get(&key) {
        map.insert(key.clone(), HashSet::new()).unwrap_or_default();
    }
    map.get_mut(&key).unwrap().insert(value);
}

pub(crate) fn run_analysis_shared<'tcx>(tcx: TyCtxt<'tcx>, path_buf: PathBuf) {
    let crate_name = format!("{}", tcx.crate_name(LOCAL_CRATE));
    let crate_id = tcx.sess.local_stable_crate_id().to_u64();

    //##############################################################################################################
    // 2. Determine which functions represent tests and store the names of those nodes on the filesystem

    let mut tests: Vec<String> = Vec::new();
    for def_id in tcx.mir_keys(()) {
        for attr in tcx.get_attrs_unchecked(def_id.to_def_id()) {
            if attr.name_or_empty().to_ident_string() == TEST_MARKER {
                tests.push(def_id_name(tcx, def_id.to_def_id()).expect_one());
            }
        }
    }

    if tests.len() > 0 {
        write_to_file(
            tests.join("\n").to_string(),
            path_buf.clone(),
            |buf| get_test_path(buf, &crate_name, crate_id),
            false,
        );
    }

    debug!("Exported tests for {}", crate_name);

    //##################################################################################################################
    // 3. Import checksums

    let checksums_path_buf = get_checksums_path(path_buf.clone(), &crate_name, crate_id);

    let maybe_checksums = read(checksums_path_buf);

    let old_checksums = {
        if let Ok(checksums) = maybe_checksums {
            Checksums::from(checksums.as_slice())
        } else {
            Checksums::new()
        }
    };

    debug!("Imported checksums for {}", crate_name);

    //##################################################################################################################
    // 4. Calculate new checksums and names of changed nodes and write this information to filesystem

    let mut new_checksums = Checksums::new();
    let mut changed_nodes = Vec::new();

    for def_id in tcx.mir_keys(()) {
        let has_body = tcx.hir().maybe_body_owned_by(*def_id).is_some();

        if has_body {
            let body = match tcx.hir().body_const_context(*def_id) {
                Some(ConstContext::ConstFn) | None => tcx.optimized_mir(*def_id),
                Some(ConstContext::Static(..)) | Some(ConstContext::Const) => {
                    tcx.mir_for_ctfe(*def_id)
                }
            };

            //##########################################################################################################
            // Check if checksum changed

            let name = tcx.def_path_debug_str(def_id.to_def_id());
            let checksum = get_checksum(tcx, body);

            let maybe_old = old_checksums.inner().get(&name);

            let changed = match maybe_old {
                Some(before) => *before != checksum,
                None => true,
            };

            if changed {
                changed_nodes.push(def_id_name(tcx, def_id.to_def_id()).expect_one());
            }
            new_checksums.inner_mut().insert(name, checksum);
        }
    }

    write_to_file(
        new_checksums.to_string().to_string(),
        path_buf.clone(),
        |buf| get_checksums_path(buf, &crate_name, crate_id),
        false,
    );

    write_to_file(
        changed_nodes.join("\n").to_string(),
        path_buf.clone(),
        |buf| get_changes_path(buf, &crate_name, crate_id),
        false,
    );

    debug!("Exported changes for {}", crate_name);

    //##################################################################################################################
    // 5. Write a mapping of reexports to file for subsequent crates

    process_reexports(tcx, path_buf.clone(), &crate_name, crate_id);
}

fn process_reexports(tcx: TyCtxt, path_buf: PathBuf, crate_name: &str, crate_id: u64) {
    let resolutions = tcx.resolutions(());

    let reexport_map = &resolutions.reexport_map;
    let mut mapping = vec![];

    for (mod_def_id, reexports) in reexport_map {
        for mod_child in reexports {
            if let Visibility::Public = mod_child.vis {
                if let Res::Def(kind, def_id) = mod_child.res {
                    if let DefKind::Mod
                    | DefKind::Fn
                    | DefKind::Struct
                    | DefKind::Enum
                    | DefKind::Trait
                    | DefKind::Ctor(..) = kind
                    {
                        let (exported_name, local_name) = match kind {
                            DefKind::Mod => {
                                let local_name = format!(
                                    "{}::{}",
                                    tcx.crate_name(def_id.krate),
                                    tcx.def_path_str(def_id)
                                );
                                let exported_name =
                                    exported_name(tcx, *mod_def_id, mod_child.ident.name);
                                (exported_name, local_name)
                            }
                            _ => {
                                let local_name = def_id_name(tcx, def_id).expect_one();
                                let exported_name =
                                    exported_name(tcx, *mod_def_id, mod_child.ident.name);
                                (exported_name, local_name)
                            }
                        };

                        trace!("Found reexport: {} as {:?}", local_name, exported_name);

                        match kind {
                            DefKind::Fn | DefKind::Ctor(..) => {
                                mapping.push((exported_name, local_name));
                            }
                            DefKind::Struct | DefKind::Enum | DefKind::Trait => {
                                mapping.push((exported_name.clone() + "!adt", local_name.clone()));
                            }
                            _ => {
                                mapping.push((exported_name + "::", local_name + "::"));
                            }
                        }
                    }
                }
            }
        }
    }

    write_to_file(
        mapping
            .iter()
            .map(|(l, e)| format!("{} | {}", l, e))
            .join("\n"),
        path_buf,
        |path_buf| get_reexports_path(path_buf, crate_name, crate_id),
        false,
    );
}
