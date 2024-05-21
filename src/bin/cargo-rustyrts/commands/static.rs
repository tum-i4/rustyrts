use std::{
    cell::RefMut,
    collections::HashSet,
    fmt::Display,
    fs::{read_dir, DirEntry},
    path::PathBuf,
};

use cargo::{core::Shell, util::command_prelude::*, CargoResult};

use internment::{Arena, ArenaIntern};
use rustyrts::{
    constants::{ENDING_CHANGES, ENDING_GRAPH, ENDING_TEST},
    fs_utils::{read_lines, read_lines_filter_map, CacheKind},
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

    fn cache_kind(&self) -> CacheKind {
        CacheKind::Static
    }

    fn default_target_dir(&self, target_dir: PathBuf) -> std::path::PathBuf {
        let mut target_dir = target_dir;
        target_dir.push("static");
        target_dir
    }

    fn select_tests(&self, config: &Config, target_dir: PathBuf) -> HashSet<String> {
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

        // if verbose {
        //     let mut complete_graph_path = path_buf.clone();
        //     complete_graph_path.push(FILE_COMPLETE_GRAPH);
        //     let mut file = match OpenOptions::new()
        //         .create(true)
        //         .write(true)
        //         .append(false)
        //         .open(complete_graph_path.as_path())
        //     {
        //         Ok(file) => file,
        //         Err(reason) => panic!("Failed to open file: {}", reason),
        //     };

        //     let checksum_files = files.iter().filter(|path| {
        //         path.file_name()
        //             .to_str()
        //             .unwrap()
        //             .ends_with(ENDING_CHECKSUM)
        //     });
        //     let mut checksums_nodes = HashSet::new();

        //     for checkums_path in checksum_files {
        //         let maybe_checksums = read(checkums_path.path());
        //         if let Ok(checksums) = maybe_checksums {
        //             let checksums = Checksums::from(checksums.as_slice());
        //             for node in checksums.keys() {
        //                 checksums_nodes.insert(node.clone());
        //             }
        //         }
        //     }

        //     match file
        //         .write_all(format!("{}\n", dependency_graph.pretty(checksums_nodes)).as_bytes())
        //     {
        //         Ok(_) => {}
        //         Err(reason) => panic!("Failed to write to file: {}", reason),
        //     };
        // }

        // Read changed nodes
        let changed_nodes = read_lines_filter_map(
            &files,
            ENDING_CHANGES,
            |_line| true,
            |line| arena.intern(line),
        );

        // Read possibly affected tests
        let tests_found =
            read_lines_filter_map(&files, ENDING_TEST, |_line| true, |line| arena.intern(line));

        let reachable_nodes = dependency_graph.reachable_nodes(changed_nodes.clone());
        let mut affected_tests: HashSet<String> = tests_found
            .intersection(&reachable_nodes)
            .map(|interned| interned.to_string())
            .collect();

        if std::env::var("RUSTYRTS_RETEST_ALL").is_ok() {
            let tests = read_lines(&files, ENDING_TEST);
            affected_tests.extend(tests);
        }

        print_stats(
            &mut *config.shell(),
            "Static RTS\n",
            &tests_found,
            &changed_nodes,
            &reachable_nodes,
            &affected_tests,
        )
        .unwrap();

        affected_tests
    }
}

pub fn exec(config: &mut Config, args: &ArgMatches) -> CliResult {
    super::exec(config, args, &StaticMode)
}

pub(crate) fn print_stats<T: Display>(
    shell: &mut Shell,
    status: T,
    tests_found: &HashSet<ArenaIntern<'_, String>>,
    changed_nodes: &HashSet<ArenaIntern<'_, String>>,
    reachable_nodes: &HashSet<ArenaIntern<'_, String>>,
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
        shell.print_ansi_stderr(format!("Reachable: {}    ", reachable_nodes.len()).as_bytes())
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
        shell.print_ansi_stderr(format!("Reachable: {:?}\n", reachable_nodes.len()).as_bytes())
    })?;
    shell.verbose(|shell| {
        shell.print_ansi_stderr(format!("Affected: {:?}\n", affected_tests).as_bytes())
    })?;

    Ok(())
}
