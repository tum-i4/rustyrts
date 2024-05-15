use std::{
    collections::HashSet,
    fs::{create_dir_all, read, read_dir, remove_file, DirEntry, OpenOptions},
    io::Write,
    path::PathBuf,
};

use cargo::util::command_prelude::*;

use internment::Arena;
use itertools::Itertools;
use rustyrts::{
    checksums::Checksums,
    constants::{ENDING_CHANGES, ENDING_CHECKSUM, ENDING_GRAPH, ENDING_TEST, FILE_COMPLETE_GRAPH},
    fs_utils::{read_lines_filter_map, CacheKind},
    static_rts::graph::DependencyGraph,
};

use super::SelectionMode;

pub fn cli() -> Command {
    subcommand("static")
        .about("Perform regression test selection using a static technique, constructing a dependency graph")
        .arg(
            Arg::new("args")
                .value_name("ARGS")
                .help("Arguments for the test binary")
                .num_args(0..)
                .last(true),
        )
        .arg_message_format()
        .arg(
            flag(
                "quiet",
                "Display one character per test instead of one line",
            )
            .short('q'),
        )
        .arg_package_spec(
            "Package to run tests for",
            "Test all packages in the workspace",
            "Exclude packages from the test",
        )
        .arg_features()
        .arg_jobs()
        .arg_unsupported_keep_going()
        .arg_release("Build artifacts in release mode, with optimizations")
        .arg_profile("Build artifacts with the specified profile")
        .arg_target_triple("Build for the target triple")
        .arg_target_dir()
        .arg_unit_graph()
        .arg_timings()
        .arg_manifest_path()
}

pub(crate) struct StaticMode;

impl SelectionMode for StaticMode {
    fn cmd(&self) -> std::path::PathBuf {
        let mut path_buf = std::env::current_exe().expect("current executable path invalid");
        path_buf.set_file_name("rustyrts-static");
        path_buf
    }

    fn default_target_dir(&self, target_dir: PathBuf) -> std::path::PathBuf {
        let mut target_dir = target_dir;
        target_dir.push("static");
        target_dir
    }

    fn select_tests(&self, config: &Config, target_dir: PathBuf) -> Vec<String> {
        let verbose = config.extra_verbose();

        let path_buf = {
            let mut target_dir = target_dir;
            target_dir.push(std::convert::Into::<&str>::into(CacheKind::Static));
            target_dir
        };

        let files: Vec<DirEntry> = read_dir(path_buf.as_path())
            .unwrap()
            .map(|maybe_path| maybe_path.unwrap())
            .collect();

        let arena = Arena::new();

        // Read graphs
        let mut dependency_graph: DependencyGraph<String> = DependencyGraph::new(&arena);

        let edges = read_lines_filter_map(
            &files,
            ENDING_GRAPH,
            |line| !line.trim_start().starts_with('\\') && line.contains("\" -> \""),
            |line| line,
        );
        dependency_graph.import_edges(edges);

        if verbose {
            let mut complete_graph_path = path_buf.clone();
            complete_graph_path.push(FILE_COMPLETE_GRAPH);
            let mut file = match OpenOptions::new()
                .create(true)
                .write(true)
                .append(false)
                .open(complete_graph_path.as_path())
            {
                Ok(file) => file,
                Err(reason) => panic!("Failed to open file: {}", reason),
            };

            let checksum_files = files.iter().filter(|path| {
                path.file_name()
                    .to_str()
                    .unwrap()
                    .ends_with(ENDING_CHECKSUM)
            });
            let mut checksums_nodes = HashSet::new();

            for checkums_path in checksum_files {
                let maybe_checksums = read(checkums_path.path());
                if let Ok(checksums) = maybe_checksums {
                    let checksums = Checksums::from(checksums.as_slice());
                    for node in checksums.keys() {
                        checksums_nodes.insert(node.clone());
                    }
                }
            }

            match file
                .write_all(format!("{}\n", dependency_graph.pretty(checksums_nodes)).as_bytes())
            {
                Ok(_) => {}
                Err(reason) => panic!("Failed to write to file: {}", reason),
            };
        }

        // Read changed nodes
        let changed_nodes = read_lines_filter_map(
            &files,
            ENDING_CHANGES,
            |_line| true,
            |line| arena.intern(line),
        );

        if verbose {
            println!(
                "Nodes that have changed:\n{}\n",
                changed_nodes
                    .iter()
                    .sorted_by(|a, b| Ord::cmp(&***a, &***b,))
                    .join(", ")
            );
        } else {
            println!("#Nodes that have changed: {}\n", changed_nodes.len());
        }

        // Read possibly affected tests
        let tests =
            read_lines_filter_map(&files, ENDING_TEST, |_line| true, |line| arena.intern(line));

        println!("#Tests that have been found: {}\n", tests.len());

        let reached_nodes = dependency_graph.reachable_nodes(changed_nodes);
        let affected_tests: HashSet<String> = tests
            .intersection(&reached_nodes)
            .map(|interned| interned.to_string())
            .collect();

        println!(
            "#Nodes that reach any changed node in the graph: {}\n",
            reached_nodes.len()
        );

        if verbose {
            println!(
                "Affected tests:\n{}\n",
                affected_tests.iter().sorted().join(", ")
            );
        } else {
            println!("#Affected tests: {}\n", affected_tests.len());
        }

        affected_tests
            .into_iter()
            .map(|s| s.clone().clone())
            .collect_vec()
    }

    fn clean_intermediate_files(&self, target_dir: PathBuf) {
        let path_buf = {
            let mut target_dir = target_dir.clone();
            target_dir.push(std::convert::Into::<&str>::into(CacheKind::Doctests));
            target_dir
        };

        create_dir_all(path_buf.as_path())
            .unwrap_or_else(|_| panic!("Failed to create directory {}", path_buf.display()));

        let path_buf = {
            let mut target_dir = target_dir;
            target_dir.push(std::convert::Into::<&str>::into(CacheKind::Static));
            target_dir
        };

        create_dir_all(path_buf.as_path())
            .unwrap_or_else(|_| panic!("Failed to create directory {}", path_buf.display()));

        if let Ok(files) = read_dir(path_buf.as_path()) {
            for path in files.flatten() {
                let file_name = path.file_name();
                if file_name.to_str().unwrap().ends_with(ENDING_CHANGES) {
                    remove_file(path.path()).unwrap();
                }
            }
        }
    }
}

pub fn exec(config: &mut Config, args: &ArgMatches) -> CliResult {
    super::exec(config, args, &StaticMode)
}
