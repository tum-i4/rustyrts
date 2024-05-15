use std::{
    collections::HashSet,
    fs::{create_dir_all, read_dir, read_to_string, remove_file, DirEntry},
    path::PathBuf,
};

use cargo::util::command_prelude::*;
use internment::{Arena, ArenaIntern};
use itertools::Itertools;
use rustyrts::{
    constants::{ENDING_CHANGES, ENDING_PROCESS_TRACE, ENDING_TEST, ENDING_TRACE},
    fs_utils::{read_lines, read_lines_filter_map, CacheKind},
};

use super::SelectionMode;

pub fn cli() -> Command {
    subcommand("dynamic")
        .about("Perform regression test selection using a dynamic technique, collecting runtime traces")
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

pub(crate) struct DynamicMode;

impl SelectionMode for DynamicMode {
    fn cmd(&self) -> std::path::PathBuf {
        let mut path_buf = std::env::current_exe().expect("current executable path invalid");
        path_buf.set_file_name("rustyrts-dynamic");
        path_buf
    }

    fn default_target_dir(&self, target_dir: PathBuf) -> std::path::PathBuf {
        let mut target_dir = target_dir;
        target_dir.push("dynamic");
        target_dir
    }

    fn select_tests(&self, config: &Config, target_dir: PathBuf) -> Vec<String> {
        let verbose = config.extra_verbose();

        let path_buf = {
            let mut target_dir = target_dir;
            target_dir.push(std::convert::Into::<&str>::into(CacheKind::Dynamic));
            target_dir
        };

        let files: Vec<DirEntry> = read_dir(path_buf.as_path())
            .unwrap()
            .map(|maybe_path| maybe_path.unwrap())
            .collect();

        let arena: Arena<String> = Arena::new();

        // Read tests
        let tests = read_lines_filter_map(&files, ENDING_TEST, |_s| true, |s| arena.intern(s));

        // Read changed nodes
        let changed_nodes =
            read_lines_filter_map(&files, ENDING_CHANGES, |_s| true, |s| arena.intern(s));

        if verbose {
            println!(
                "Nodes that have changed:\n{}\n",
                changed_nodes
                    .iter()
                    .sorted_by(|a, b| Ord::cmp(&***a, &***b))
                    .join(", ")
            );
        } else {
            println!("#Nodes that have changed: {}\n", changed_nodes.len());
        }

        println!("#Tests that have been found: {}\n", tests.len());

        // Read traces or dependencies
        let ending = ENDING_TRACE;

        let mut affected_tests: Vec<String> = Vec::new();

        let analyzed_tests: Vec<&DirEntry> = files
            .iter()
            .filter(|traces| {
                traces
                    .file_name()
                    .to_os_string()
                    .into_string()
                    .unwrap()
                    .ends_with(ending)
            })
            .collect();

        let analyzed_tests_names: HashSet<ArenaIntern<'_, String>> = analyzed_tests
            .iter()
            .map(|f| {
                arena.intern(
                    f.file_name()
                        .into_string()
                        .unwrap()
                        .split_once('.')
                        .unwrap()
                        .0
                        .split_once("::")
                        .unwrap()
                        .1
                        .to_string(),
                )
            })
            .collect();

        println!("#Tests with information: {}\n", analyzed_tests_names.len());

        affected_tests.append(
            &mut tests
                .difference(&analyzed_tests_names)
                .map(|s| (**s).clone())
                .collect_vec(),
        );

        for file in analyzed_tests {
            let traced_nodes: HashSet<ArenaIntern<String>> = read_to_string(file.path())
                .unwrap()
                .split('\n')
                .filter(|s| !s.is_empty())
                .map(|s| arena.intern(s.to_string()))
                .collect();

            let mut intersection = traced_nodes.intersection(&changed_nodes);
            if intersection.next().is_some() {
                let test_name = file
                    .file_name()
                    .into_string()
                    .unwrap()
                    .split_once('.')
                    .unwrap()
                    .0
                    .split_once("::")
                    .unwrap()
                    .1
                    .to_string();
                affected_tests.push(test_name);
            }
        }

        if verbose {
            println!(
                "Affected tests:\n{}\n",
                affected_tests.iter().sorted().join(", ")
            );
        } else {
            println!("#Affected tests: {}\n", affected_tests.len());
        }

        if std::env::var("RUSTYRTS_RETEST_ALL").is_ok() {
            let tests = read_lines(&files, ENDING_TEST);
            affected_tests = tests.into_iter().collect_vec();
        }

        affected_tests
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
            target_dir.push(std::convert::Into::<&str>::into(CacheKind::Dynamic));
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

                #[cfg(unix)]
                if file_name.to_str().unwrap().ends_with(ENDING_PROCESS_TRACE) {
                    remove_file(path.path()).unwrap();
                }
            }
        }
    }
}

pub fn exec(config: &mut Config, args: &ArgMatches) -> CliResult {
    super::exec(config, args, &DynamicMode)
}
