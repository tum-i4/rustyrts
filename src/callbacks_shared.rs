use itertools::Itertools;
use log::{debug, trace};
use rustc_hir::{
    def::{DefKind, Res},
    def_id::LOCAL_CRATE,
    ConstContext,
};
use rustc_middle::ty::{TyCtxt, Visibility};
use std::{fs::read, path::PathBuf};

use crate::{
    checksums::Checksums,
    fs_utils::{
        get_changes_path, get_checksums_ctfe_path, get_checksums_path, get_reexports_path,
        get_test_path, write_to_file,
    },
    names::{def_id_name, exported_name},
    static_rts::callback::PATH_BUF,
};

const EXCLUDED_CRATES: &[&str] = &["build_script_build", "build_script_main"];

pub(crate) const TEST_MARKER: &str = "rustc_test_marker";

pub(crate) fn excluded<'tcx>(tcx: TyCtxt<'tcx>) -> bool {
    let local_crate_name = tcx.crate_name(LOCAL_CRATE);
    EXCLUDED_CRATES
        .iter()
        .any(|krate| *krate == local_crate_name.as_str())
}

pub(crate) fn run_analysis_shared<'tcx>(
    tcx: TyCtxt<'tcx>,
    mut new_checksums: Checksums,
    mut new_checksums_ctfe: Checksums,
) {
    let crate_name = format!("{}", tcx.crate_name(LOCAL_CRATE));
    let crate_id = tcx.sess.local_stable_crate_id().to_u64();

    //##############################################################################################################
    // 2. Determine which functions represent tests and store the names of those nodes on the filesystem

    let mut tests: Vec<String> = Vec::new();
    for def_id in tcx.mir_keys(()) {
        for attr in tcx.get_attrs_unchecked(def_id.to_def_id()) {
            if attr.name_or_empty().to_ident_string() == TEST_MARKER {
                tests.push(def_id_name(tcx, def_id.to_def_id()));
            }
        }
    }

    if tests.len() > 0 {
        write_to_file(
            tests.join("\n").to_string(),
            PATH_BUF.get().unwrap().clone(),
            |buf| get_test_path(buf, &crate_name, crate_id),
            false,
        );
    }

    debug!("Exported tests for {}", crate_name);

    //##################################################################################################################
    // 3. Import checksums

    let old_checksums = {
        let checksums_path_buf =
            get_checksums_path(PATH_BUF.get().unwrap().clone(), &crate_name, crate_id);

        let maybe_checksums = read(checksums_path_buf);

        if let Ok(checksums) = maybe_checksums {
            Checksums::from(checksums.as_slice())
        } else {
            Checksums::new()
        }
    };

    let old_checksums_ctfe = {
        let checksums_path_buf =
            get_checksums_ctfe_path(PATH_BUF.get().unwrap().clone(), &crate_name, crate_id);

        let maybe_checksums = read(checksums_path_buf);

        if let Ok(checksums) = maybe_checksums {
            Checksums::from(checksums.as_slice())
        } else {
            Checksums::new()
        }
    };

    debug!("Imported checksums for {}", crate_name);

    //##################################################################################################################
    // 4. Calculate new checksums and names of changed nodes and write this information to filesystem

    let mut changed_nodes = Vec::new();

    for def_id in tcx.mir_keys(()) {
        let has_body = tcx.hir().maybe_body_owned_by(*def_id).is_some();

        if has_body {
            let name = def_id_name(tcx, def_id.to_def_id());
            let changed = match tcx.hir().body_const_context(*def_id) {
                Some(ConstContext::ConstFn) | None => {
                    let maybe_new = new_checksums.inner().get(&name);
                    let maybe_old = old_checksums.inner().get(&name);

                    match (maybe_new, maybe_old) {
                        (None, None) => unreachable!(),
                        (None, Some(checksums)) => {
                            new_checksums
                                .inner_mut()
                                .insert(name.clone(), checksums.clone());
                            false
                        }
                        (Some(_), None) => true,
                        (Some(new), Some(old)) => new != old,
                    }
                }
                Some(ConstContext::Static(..)) | Some(ConstContext::Const) => {
                    let maybe_new = new_checksums_ctfe.inner().get(&name);
                    let maybe_old = old_checksums_ctfe.inner().get(&name);

                    match (maybe_new, maybe_old) {
                        (None, None) => unreachable!(),
                        (None, Some(checksums)) => {
                            new_checksums_ctfe
                                .inner_mut()
                                .insert(name.clone(), checksums.clone());
                            false
                        }
                        (Some(_), None) => true,
                        (Some(new), Some(old)) => new != old,
                    }
                }
            };

            if changed {
                changed_nodes.push(name);
            }
        }
    }

    write_to_file(
        new_checksums.to_string().to_string(),
        PATH_BUF.get().unwrap().clone(),
        |buf| get_checksums_path(buf, &crate_name, crate_id),
        false,
    );

    write_to_file(
        new_checksums_ctfe.to_string().to_string(),
        PATH_BUF.get().unwrap().clone(),
        |buf| get_checksums_ctfe_path(buf, &crate_name, crate_id),
        false,
    );

    write_to_file(
        changed_nodes.join("\n").to_string(),
        PATH_BUF.get().unwrap().clone(),
        |buf| get_changes_path(buf, &crate_name, crate_id),
        false,
    );

    debug!("Exported changes for {}", crate_name);

    //##################################################################################################################
    // 5. Write a mapping of reexports to file for subsequent crates

    process_reexports(tcx, PATH_BUF.get().unwrap().clone(), &crate_name, crate_id);
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
                                let local_name = def_id_name(tcx, def_id);
                                let exported_name =
                                    exported_name(tcx, *mod_def_id, mod_child.ident.name);
                                (exported_name, local_name)
                            }
                        };

                        if !exported_name.ends_with("_") {
                            trace!(
                                "Found reexport: {} as {:?}",
                                local_name,
                                exported_name.clone()
                            );

                            match kind {
                                DefKind::Fn | DefKind::Ctor(..) => {
                                    mapping.push((exported_name, local_name));
                                }
                                DefKind::Struct | DefKind::Enum | DefKind::Trait => {
                                    mapping.push((exported_name + "!adt", local_name));
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
