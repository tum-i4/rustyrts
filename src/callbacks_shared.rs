use itertools::Itertools;
use log::debug;
use rustc_hir::def_id::LOCAL_CRATE;
use rustc_middle::ty::{
    Binder, ExistentialTraitRef, GenericParamDefKind, ImplSubject, Instance, InstanceDef,
    InternalSubsts, List, ParamEnv, TyCtxt, VtblEntry,
};
use rustc_trait_selection::traits::impossible_predicates;
use std::{collections::HashSet, fs::read};

use crate::{
    checksums::{get_checksum_instance, insert_hashmap, Checksums},
    fs_utils::{
        get_changes_path, get_checksums_ctfe_path, get_checksums_path, get_test_path, write_to_file,
    },
    names::def_id_name,
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
    new_checksums_ctfe: Checksums,
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

    for (_, impls) in tcx.all_local_trait_impls(()) {
        for def_id in impls {
            if let Some(binder) = tcx.impl_trait_ref(def_id.to_def_id()) {
                let trait_ref = binder.0;
                debug!("Trying for {:?} - {:?}", def_id, trait_ref);

                // Code adapted from: https://doc.rust-lang.org/stable/nightly-rustc/src/rustc_trait_selection/traits/vtable.rs.html#218

                let own_existential_entries = tcx.own_existential_vtable_entries(trait_ref.def_id);

                let instances = own_existential_entries
                    .iter()
                    .copied()
                    .map(|def_id| {
                        let substs =
                            InternalSubsts::for_item(tcx, def_id, |param, _| match param.kind {
                                GenericParamDefKind::Lifetime => tcx.lifetimes.re_erased.into(),
                                GenericParamDefKind::Type { .. }
                                | GenericParamDefKind::Const { .. } => {
                                    trait_ref.substs[param.index as usize]
                                }
                            });

                        let predicates = tcx.predicates_of(def_id).instantiate_own(tcx, substs);
                        if impossible_predicates(
                            tcx,
                            predicates.map(|(first, _)| first).collect_vec(),
                        ) {
                            return None;
                        }

                        Instance::resolve_for_vtable(tcx, ParamEnv::reveal_all(), def_id, substs)
                    })
                    .filter(|o| o.is_some())
                    .map(|o| o.unwrap());

                for instance in instances {
                    let def_id = match instance.def {
                        InstanceDef::Item(item) => item.did,
                        InstanceDef::Intrinsic(def_id) => def_id,
                        InstanceDef::VTableShim(def_id) => def_id,
                        InstanceDef::ReifyShim(def_id) => def_id,
                        InstanceDef::FnPtrShim(def_id, _) => def_id,
                        InstanceDef::Virtual(def_id, _) => def_id,
                        InstanceDef::ClosureOnceShim {
                            call_once,
                            track_caller: _,
                        } => call_once,
                        InstanceDef::DropGlue(def_id, _) => def_id,
                        InstanceDef::CloneShim(def_id, _) => def_id,
                    };
                    let name = def_id_name(tcx, def_id);
                    let checksum = get_checksum_instance(tcx, &instance);
                    debug!("Considering {:?} in checksums of {}", instance, name);
                    insert_hashmap(new_checksums.inner_mut(), name, checksum)
                }
            }
        }
    }

    let mut changed_nodes = Vec::new();

    let mut names = HashSet::new();
    names.extend(new_checksums.inner().keys().map(|s| s.clone()));
    names.extend(old_checksums.inner().keys().map(|s| s.clone()));

    for name in names {
        let changed = {
            let maybe_new = new_checksums.inner().get(&name);
            let maybe_old = old_checksums.inner().get(&name);

            match (maybe_new, maybe_old) {
                (None, None) => unreachable!(),
                (None, Some(_)) => true,
                (Some(_), None) => true,
                (Some(new), Some(old)) => new != old,
            }
        };
        if changed {
            changed_nodes.push(name.clone());
        }
    }

    let mut names_ctfe = HashSet::new();
    names_ctfe.extend(new_checksums_ctfe.inner().keys().map(|s| s.clone()));
    names_ctfe.extend(old_checksums_ctfe.inner().keys().map(|s| s.clone()));

    for name in names_ctfe {
        let changed = {
            let maybe_new = new_checksums_ctfe.inner().get(&name);
            let maybe_old = old_checksums_ctfe.inner().get(&name);

            match (maybe_new, maybe_old) {
                (None, None) => unreachable!(),
                (None, Some(_)) => true,
                (Some(_), None) => true,
                (Some(new), Some(old)) => new != old,
            }
        };

        if changed {
            changed_nodes.push(name.clone());
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
}
