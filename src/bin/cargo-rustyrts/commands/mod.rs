use std::{
    collections::{HashMap, HashSet},
    fs::{create_dir_all, read_dir, remove_file, rename, DirEntry},
    path::{Path, PathBuf},
    vec::Vec,
};

use cargo::{
    core::{compiler::Compilation, Shell, Workspace},
    util::Filesystem,
};
use internment::{Arena, ArenaIntern};
use itertools::Itertools;
use rustyrts::{
    callbacks_shared::{calculate_changes, import_checksums, DOCTEST_PREFIX},
    constants::{ENDING_CHANGES, ENDING_GRAPH},
    fs_utils::CacheKind,
    static_rts::graph::DependencyGraph,
};
use rustyrts::{
    constants::{
        ENDING_CHECKSUM, ENDING_CHECKSUM_CONST, ENDING_CHECKSUM_CONST_OLD, ENDING_CHECKSUM_OLD,
        ENDING_CHECKSUM_VTBL, ENDING_CHECKSUM_VTBL_OLD, ENDING_PROCESS_TRACE,
    },
    fs_utils::read_lines_filter_map,
};

use crate::{
    command_prelude::*, commands::r#static::print_stats, doctest_rts::run_analysis_doctests,
};

pub fn commands() -> Vec<Command> {
    vec![r#static::cli(), dynamic::cli(), clean::cli()]
}

pub type Exec = fn(&mut Config, &ArgMatches) -> CliResult;

pub fn command_exec(cmd: &str) -> Option<Exec> {
    let f = match cmd {
        "static" => r#static::exec,
        "dynamic" => dynamic::exec,
        "clean" => clean::exec,
        _ => return None,
    };
    Some(f)
}

pub(crate) mod clean;
pub(crate) mod dynamic;
pub(crate) mod r#static;

pub trait SelectionMode {
    fn cmd(&self) -> PathBuf;

    fn cache_kind(&self) -> CacheKind;

    fn default_target_dir(&self, target_dir: PathBuf) -> PathBuf;

    fn select_tests(&self, config: &Config, target_dir: PathBuf) -> HashSet<String>;

    fn prepare_intermediate_files(&self, path: &Path) {
        create_dir_all(path)
            .unwrap_or_else(|_| panic!("Failed to create directory {}", path.display()));

        if let Ok(files) = read_dir(path) {
            for dir_entry in files.flatten() {
                let file_name = &dir_entry.file_name();
                let file_name = file_name.to_str().unwrap();

                if file_name.ends_with(ENDING_CHANGES) {
                    remove_file(dir_entry.path()).unwrap();
                }

                #[cfg(unix)]
                if file_name.ends_with(ENDING_PROCESS_TRACE) {
                    remove_file(dir_entry.path()).unwrap();
                }

                if !file_name.contains("]") {
                    if let Some(name) = file_name.strip_suffix(ENDING_GRAPH) {
                        remove_file(dir_entry.path()).unwrap();
                    }
                    if let Some(name) = file_name.strip_suffix(ENDING_CHECKSUM) {
                        let mut new_path = dir_entry.path();
                        new_path.set_extension(ENDING_CHECKSUM_OLD);
                        rename(&dir_entry.path(), &new_path).unwrap();
                    }
                    if let Some(name) = file_name.strip_suffix(ENDING_CHECKSUM_VTBL) {
                        let mut new_path = dir_entry.path();
                        new_path.set_extension(ENDING_CHECKSUM_VTBL_OLD);
                        rename(&dir_entry.path(), &new_path).unwrap();
                    }
                    if let Some(name) = file_name.strip_suffix(ENDING_CHECKSUM_CONST) {
                        let mut new_path = dir_entry.path();
                        new_path.set_extension(ENDING_CHECKSUM_CONST_OLD);
                        rename(&dir_entry.path(), &new_path).unwrap();
                    }
                }
            }
        }
    }

    fn clean_intermediate_files(&self, path: &Path) {
        if let Ok(files) = read_dir(path) {
            for dir_entry in files.flatten() {
                let file_name = dir_entry.file_name();
                let file_name = file_name.to_str().unwrap();

                if let Some(name) = file_name.strip_suffix(ENDING_CHECKSUM_OLD) {
                    remove_file(dir_entry.path()).unwrap();
                }
                if let Some(name) = file_name.strip_suffix(ENDING_CHECKSUM_VTBL_OLD) {
                    remove_file(dir_entry.path()).unwrap();
                }
                if let Some(name) = file_name.strip_suffix(ENDING_CHECKSUM_CONST_OLD) {
                    remove_file(dir_entry.path()).unwrap();
                }
            }
        }
    }

    fn select_doc_tests(
        &self,
        ws: &Workspace<'_>,
        test_args: &[String],
        compilation: &Compilation,
        target_dir: PathBuf,
    ) -> HashSet<String> {
        let config = ws.config();

        // Updates the graph
        let test_names =
            run_analysis_doctests(ws, test_args, compilation).expect("Failed to analyze doc tests");

        let path_buf = {
            let mut target_dir = target_dir;
            target_dir.push(std::convert::Into::<&str>::into(CacheKind::Doctests));
            target_dir
        };

        let files: Vec<DirEntry> = read_dir(path_buf.as_path())
            .unwrap()
            .map(|maybe_path| maybe_path.unwrap())
            .collect();

        let arena = Arena::new();

        let mut changed_nodes = HashSet::new();
        for file in files
            .iter()
            .filter(|entry| !entry.file_name().to_str().unwrap().contains("]"))
            .filter_map(|entry| entry.file_name().to_str().map(|s| s.to_string()))
            .filter(|name| name.ends_with(ENDING_CHECKSUM))
            .filter_map(|name| name.strip_suffix(ENDING_CHECKSUM).map(|s| s.to_string()))
            .filter_map(|name| name.strip_suffix(".").map(|s| s.to_string()))
        {
            let crate_name = file.to_string();
            let crate_id = None;

            let old_checksums =
                import_checksums(path_buf.clone(), &crate_name, crate_id, ENDING_CHECKSUM_OLD);
            let old_checksums_vtbl = import_checksums(
                path_buf.clone(),
                &crate_name,
                crate_id,
                ENDING_CHECKSUM_VTBL_OLD,
            );
            let old_checksums_const = import_checksums(
                path_buf.clone(),
                &crate_name,
                crate_id,
                ENDING_CHECKSUM_CONST_OLD,
            );
            let new_checksums =
                import_checksums(path_buf.clone(), &crate_name, crate_id, ENDING_CHECKSUM);
            let new_checksums_vtbl = import_checksums(
                path_buf.clone(),
                &crate_name,
                crate_id,
                ENDING_CHECKSUM_VTBL,
            );
            let new_checksums_const = import_checksums(
                path_buf.clone(),
                &crate_name,
                crate_id,
                ENDING_CHECKSUM_CONST,
            );

            let changed = calculate_changes(
                true,
                &old_checksums,
                &old_checksums_vtbl,
                &old_checksums_const,
                &new_checksums,
                &new_checksums_vtbl,
                &new_checksums_const,
            );
            changed_nodes.extend(
                changed
                    .into_iter()
                    .map(|s| Arena::<String>::intern(&arena, s)),
            );
        }

        // Read graphs
        let mut dependency_graph: DependencyGraph<String> = DependencyGraph::new(&arena);

        let edges = read_lines_filter_map(
            &files,
            ENDING_GRAPH,
            |line| !line.trim_start().starts_with('\\') && line.contains("\" -> \""),
            |line| line,
        );
        dependency_graph.import_edges(edges);

        // Read changed nodes
        let changed_nodes_from_libs = read_lines_filter_map(
            &files,
            ENDING_CHANGES,
            |_line| true,
            |line| arena.intern(line),
        );
        changed_nodes.extend(changed_nodes_from_libs);

        let tests_found: HashMap<ArenaIntern<'_, String>, String> = {
            let mut map = HashMap::new();
            for test_name in test_names {
                let (trimmed_test_name, fn_name) = convert_test_name(&test_name);
                let interned = Arena::<String>::intern(&arena, fn_name);
                map.insert(interned, trimmed_test_name);
            }
            map
        };

        let reachable_nodes = dependency_graph.reachable_nodes(changed_nodes.clone());
        let affected_functions: HashSet<ArenaIntern<'_, String>> =
            HashSet::from_iter(tests_found.keys().cloned())
                .intersection(&reachable_nodes)
                .copied()
                .collect();

        let affected_tests = {
            let mut set = HashSet::new();
            for affected in affected_functions {
                set.insert(tests_found.get(&affected).unwrap().to_string());
            }
            set
        };

        print_stats(
            &mut config.shell(),
            "Static RTS (doctests)\n",
            &tests_found.keys().cloned().collect(),
            &changed_nodes,
            &reachable_nodes,
            &affected_tests,
        )
        .unwrap();

        affected_tests
    }
}

fn convert_test_name(test_name: &str) -> (String, String) {
    let (trimmed, _) = test_name.split_once(" - ").unwrap();
    let fn_name = DOCTEST_PREFIX.to_string()
        + &trimmed
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .collect::<String>();
    (trimmed.to_string(), fn_name)
}

pub fn exec(config: &mut Config, args: &ArgMatches, mode: &dyn SelectionMode) -> CliResult {
    let ws = {
        let mut ws = args.workspace(config)?;

        if config.target_dir().unwrap().is_none() {
            let target_dir = mode.default_target_dir(ws.target_dir().into_path_unlocked());
            ws.set_target_dir(Filesystem::new(target_dir));
        }

        ws
    };
    let target_dir = ws.target_dir().into_path_unlocked();

    let mut compile_opts = args.compile_options(
        config,
        CompileMode::Test,
        Some(&ws),
        ProfileChecking::Custom,
    )?;

    compile_opts.build_config.requested_profile =
        args.get_profile_name(config, "test", ProfileChecking::Custom)?;

    crate::ops::run_tests(&ws, &compile_opts, target_dir, &[], mode)
}
