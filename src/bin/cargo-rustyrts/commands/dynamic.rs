use std::{
    cell::RefMut,
    collections::HashSet,
    fmt::Display,
    fs::{read_dir, read_to_string, DirEntry},
    path::PathBuf,
};

use cargo::{core::Shell, util::command_prelude::*, CargoResult};
use internment::{Arena, ArenaIntern};

use rustyrts::{
    constants::{ENDING_CHANGES, ENDING_TEST, ENDING_TRACE},
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

    fn cache_kind(&self) -> CacheKind {
        CacheKind::Dynamic
    }

    fn default_target_dir(&self, target_dir: PathBuf) -> std::path::PathBuf {
        let mut target_dir = target_dir;
        target_dir.push("dynamic");
        target_dir
    }

    fn select_tests(&self, config: &Config, target_dir: PathBuf) -> HashSet<String> {
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
        let tests_found =
            read_lines_filter_map(&files, ENDING_TEST, |_s| true, |s| arena.intern(s));

        // Read changed nodes
        let changed_nodes =
            read_lines_filter_map(&files, ENDING_CHANGES, |_s| true, |s| arena.intern(s));

        // Read traces or dependencies
        let mut affected_tests = HashSet::new();

        let analyzed_tests: Vec<&DirEntry> = files
            .iter()
            .filter(|traces| {
                traces
                    .file_name()
                    .to_os_string()
                    .into_string()
                    .unwrap()
                    .ends_with(ENDING_TRACE)
            })
            .collect();

        let traced_tests: HashSet<ArenaIntern<'_, String>> = analyzed_tests
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

        println!("#Tests with information: {}\n", traced_tests.len());

        affected_tests.extend(tests_found.difference(&traced_tests).map(|s| (**s).clone()));

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
                affected_tests.insert(test_name);
            }
        }

        if std::env::var("RUSTYRTS_RETEST_ALL").is_ok() {
            let tests = read_lines(&files, ENDING_TEST);
            affected_tests.extend(tests);
        }

        print_stats(
            &mut *config.shell(),
            "Dynamic RTS\n",
            &tests_found,
            &traced_tests,
            &changed_nodes,
            &affected_tests,
        )
        .unwrap();

        affected_tests
    }
}

pub fn exec(config: &mut Config, args: &ArgMatches) -> CliResult {
    super::exec(config, args, &DynamicMode)
}

pub(crate) fn print_stats<T: Display>(
    shell: &mut Shell,
    status: T,
    tests_found: &HashSet<ArenaIntern<'_, String>>,
    traced_tests: &HashSet<ArenaIntern<'_, String>>,
    changed_nodes: &HashSet<ArenaIntern<'_, String>>,
    affected_tests: &HashSet<String>,
) -> CargoResult<()> {
    shell.status_header(status)?;

    shell.concise(|shell| {
        shell.print_ansi_stderr(format!("Tests found: {}    ", tests_found.len()).as_bytes())
    })?;
    shell.concise(|shell| {
        shell.print_ansi_stderr(format!("Changed: {}    ", changed_nodes.len()).as_bytes())
    })?;
    shell.concise(|shell| {
        shell.print_ansi_stderr(format!("Traced tests: {}    ", traced_tests.len()).as_bytes())
    })?;
    shell.concise(|shell| {
        shell.print_ansi_stderr(format!("Affected: {}\n", affected_tests.len()).as_bytes())
    })?;

    shell.verbose(|shell| {
        shell.print_ansi_stderr(format!("Tests found: {:?}\n", tests_found).as_bytes())
    })?;
    shell.verbose(|shell| {
        shell.print_ansi_stderr(format!("Changed: {:?}\n", changed_nodes).as_bytes())
    })?;
    shell.verbose(|shell| {
        shell.print_ansi_stderr(format!("Traced tests: {:?}\n", traced_tests).as_bytes())
    })?;
    shell.verbose(|shell| {
        shell.print_ansi_stderr(format!("Affected: {:?}\n", affected_tests).as_bytes())
    })?;

    Ok(())
}
