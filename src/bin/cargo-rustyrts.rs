use itertools::Itertools;
use rustyrts::checksums::Checksums;
use rustyrts::constants::{
    DESC_FLAG, ENDING_CHANGES, ENDING_CHECKSUM, ENDING_GRAPH, ENDING_PROCESS_TRACE, ENDING_TEST,
    ENDING_TRACE, ENV_RUSTC_WRAPPER, ENV_RUSTYRTS_ARGS, ENV_RUSTYRTS_MODE, ENV_RUSTYRTS_VERBOSE,
    ENV_TARGET_DIR, FILE_COMPLETE_GRAPH,
};
use rustyrts::fs_utils::{
    get_dynamic_path, get_static_path, get_target_dir, read_lines, read_lines_filter_map,
};
use rustyrts::static_rts::graph::DependencyGraph;
use rustyrts::utils;
use serde_json;
use std::collections::HashSet;
use std::ffi::OsString;
use std::fs::{
    create_dir_all, read, read_dir, read_to_string, remove_dir_all, remove_file, DirEntry,
    OpenOptions,
};
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use std::str::FromStr;

//######################################################################################################################
// This file is heavily inspired by rust-mir-checker
// Source: https://github.com/lizhuohua/rust-mir-checker/blob/86c3c26e797d3e25a38044fa98b765c5d220e4ea/src/bin/cargo-mir-checker.rs
//######################################################################################################################

const CARGO_RUSTYRTS_HELP: &str = r#"Static regression test selection based on the MIR

Usage:
    * cargo rustyrts clean
    * cargo rustyrts {static|dynamic}
"#;

//######################################################################################################################
// Utility functions

fn show_help() {
    println!("{}", CARGO_RUSTYRTS_HELP);
}

fn show_version() {
    println!("rust-rustyrts {}", env!("CARGO_PKG_VERSION"));
}

fn show_error(msg: String) -> ! {
    eprintln!("fatal error: {}", msg);
    std::process::exit(1)
}

// Determines whether a flag `name` is present before `--`.
// For example, has_arg_flag("-v")
fn has_arg_flag(name: &str) -> bool {
    let mut args = std::env::args().take_while(|val| val != "--");
    args.any(|val| val == name)
}

fn get_args_build() -> impl Iterator<Item = String> {
    let args = std::env::args().skip_while(|val| val != "--").skip(1);
    args.take_while(|val| val != "--")
}

fn get_args_rustc() -> impl Iterator<Item = String> {
    let args = std::env::args()
        .skip_while(|val| val != "--")
        .skip(1)
        .skip_while(|val| val != "--")
        .skip(1);
    args.take_while(|val| val != "--")
}

fn get_args_test() -> impl Iterator<Item = String> {
    let mut args: Vec<String> = std::env::args()
        .skip_while(|val| val != "--")
        .skip(1)
        .skip_while(|val| val != "--")
        .skip(1)
        .skip_while(|val| val != "--")
        .skip(1)
        .collect();

    if has_arg_flag("--json") {
        for arg in ["--", "-Zunstable-options", "--format=json", "--report-time"] {
            if !args.iter().any(|s| s == arg) {
                args.push(arg.to_string());
            }
        }
    }

    args.into_iter()
}

//######################################################################################################################
// Command helpers

fn rustyrts_static() -> Command {
    let mut path = std::env::current_exe().expect("current executable path invalid");
    path.set_file_name("rustyrts-static");
    Command::new(path)
}

fn rustyrts_dynamic() -> Command {
    let mut path = std::env::current_exe().expect("current executable path invalid");
    path.set_file_name("rustyrts-dynamic");
    Command::new(path)
}

fn cargo() -> Command {
    Command::new(std::env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo")))
}

#[derive(PartialEq)]
enum Mode {
    Clean,
    Dynamic,
    Static,
}

impl ToString for Mode {
    fn to_string(&self) -> String {
        match self {
            Mode::Clean => "clean".to_string(),
            Mode::Dynamic => "dynamic".to_string(),
            Mode::Static => "static".to_string(),
        }
    }
}

impl FromStr for Mode {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "clean" => Ok(Mode::Clean),
            "dynamic" => Ok(Mode::Dynamic),
            "static" => Ok(Mode::Static),
            _ => Err(()),
        }
    }
}

/// This will create a command `cargo test --no-run --tests --examples`
///
/// And set the following environment variables:
/// * [`ENV_RUSTYRTS_MODE`] is set to either "dynamic" or "static"
/// * [`ENV_TARGET_DIR`] is set to a custom build directory inside the directory of the cargo project
/// * [`ENV_RUSTYRTS_ARGS`] is set to the user-provided arguments for `rustc`
/// * [`ENV_RUSTC_WRAPPER`] is set to `cargo-rustyrts` itself so the execution will proceed in the second branch in main()
fn cargo_build(mode: Mode) -> Command {
    // Now we run `cargo build $FLAGS $ARGS`, giving the user the
    // chance to add additional arguments. `FLAGS` is set to identify
    // this target.  The user gets to control what gets actually passed to rustyrts.
    let mut cmd = cargo();
    cmd.arg("test");
    cmd.arg("--no-run");

    //cmd.arg("--profile");
    //cmd.arg("test");

    // cmd.arg("--lib");
    // cmd.arg("--bins");
    cmd.arg("--tests");
    cmd.arg("--examples");

    // we do not want to execute benches,
    // because they do not rely on the test harness and are not recognized aas tests
    //cmd.arg("--benches");

    for arg in get_args_build() {
        cmd.arg(arg);
    }

    // Store mode in env variable
    cmd.env(ENV_RUSTYRTS_MODE, mode.to_string());

    // Serialize the remaining args into a special environment variable.
    // This will be read by `inside_cargo_rustc` when we go to invoke
    // our actual target crate.
    let args_vec: Vec<String> = get_args_rustc().collect();
    cmd.env(
        ENV_RUSTYRTS_ARGS,
        serde_json::to_string(&args_vec).expect("failed to serialize args"),
    );

    // If not set manually, set cargo target dir to something other than the default
    cmd.env(ENV_TARGET_DIR, get_target_dir(&mode.to_string()));

    // Replace the rustc executable through RUSTC_WRAPPER environment variable
    let path = std::env::current_exe().expect("current executable path invalid");
    cmd.env(ENV_RUSTC_WRAPPER, path);

    cmd
}

/// This will create a command `cargo test --no-fail-fast --tests --examples`
///
/// And set the following environment variables:
/// * [`ENV_RUSTYRTS_MODE`] is set to either "dynamic" or "static"
/// * [`ENV_TARGET_DIR`] is set to a custom build directory inside the directory of the cargo project
/// * [`ENV_RUSTYRTS_ARGS`] is set to the user-provided arguments for `rustc`
/// * [`ENV_RUSTC_WRAPPER`] is set to `cargo-rustyrts` itself so the execution will proceed in the second branch in main()
fn cargo_test<'a, I>(mode: Mode, affected_tests: I) -> Command
where
    I: Iterator<Item = &'a String>,
{
    let mut cmd = cargo();
    cmd.arg("test");
    cmd.arg("--no-fail-fast"); // Do not stop if a test fails, execute all included tests

    // Serialize the args for rust_c into a special environemt variable.
    // This will be read by `inside_cargo_rustc` when we go to invoke
    // our actual target crate.
    let args_vec: Vec<String> = get_args_rustc().collect();
    cmd.env(
        ENV_RUSTYRTS_ARGS,
        serde_json::to_string(&args_vec).expect("failed to serialize args"),
    );

    // If not set manually, set cargo target dir to something other than the default
    cmd.env(ENV_TARGET_DIR, get_target_dir(&mode.to_string()));

    // Replace the rustc executable through RUSTC_WRAPPER environment variable
    let path = std::env::current_exe().expect("current executable path invalid");
    cmd.env(ENV_RUSTC_WRAPPER, path);

    // Store mode in env variable
    cmd.env(ENV_RUSTYRTS_MODE, mode.to_string());

    let mut affected_tests_iter = affected_tests
        .map(|line| {
            let test = line.split_once("::").unwrap().1; //.split_once("::").unwrap().1;
            test.to_string()
        })
        .peekable();

    // cmd.arg("--lib");
    // cmd.arg("--bins");
    cmd.arg("--tests");
    cmd.arg("--examples");

    // we do not want to execute benches,
    // because they do not rely on the test harness and are not recognized aas tests
    //cmd.arg("--benches");

    if affected_tests_iter.peek().is_none() && !(mode == Mode::Dynamic && has_arg_flag(DESC_FLAG)) {
        cmd.arg("--no-run");

        for arg in get_args_test() {
            cmd.arg(arg);
        }
    } else {
        let mut delimiter_found = false;
        for arg in get_args_test() {
            delimiter_found |= arg == "--";
            cmd.arg(arg);
        }

        if !delimiter_found {
            cmd.arg("--");
        }

        cmd.arg("--exact");
        for test in affected_tests_iter {
            cmd.arg(test);
        }

        if mode == Mode::Dynamic && has_arg_flag(DESC_FLAG) {
            cmd.arg("--");
            cmd.arg(DESC_FLAG);
        }
    }

    cmd
}

/// This will execute a command
///
/// And set the following environment variables:
/// * `RUSTYRTS_VERBOSE` is set if `-v` is provided
fn execute(mut cmd: Command) {
    if has_arg_flag("-vv") {
        cmd.env(ENV_RUSTYRTS_VERBOSE, ""); // this makes `inside_cargo_rustc` verbose.
        eprintln!("+ {:?}", cmd);
    }

    // Execute cmd
    match cmd.status() {
        Ok(exit) => {
            if !exit.success() {
                std::process::exit(exit.code().unwrap_or(42));
            }
        }
        Err(ref e) => panic!("error during rustyrts run: {:?}", e),
    }
}

//######################################################################################################################
// Main function

fn main() {
    // Check for version and help flags even when invoked as `cargo-rustyrts`.
    if std::env::args().any(|a| a == "--help" || a == "-h") {
        show_help();
        return;
    }
    if std::env::args().any(|a| a == "--version" || a == "-V") {
        show_version();
        return;
    }

    if let Some("rustyrts") = std::env::args().nth(1).as_ref().map(AsRef::as_ref) {
        let mode_string = std::env::args().nth(2).unwrap_or("".to_string());
        let mode = FromStr::from_str(&mode_string).ok();

        match mode {
            Some(Mode::Clean) => {
                clean();
            }
            Some(Mode::Static) => {
                // This arm is for when `cargo rustyrts static` is called. We call `cargo build`,
                // but with the `RUSTC` env var set to the `cargo-rustyrts` binary so that we come back in the other branch,
                // and dispatch the invocations to `rustyrts-static`, respectively.
                run_cargo_rustc_static();
                select_and_execute_tests_static();
            }
            Some(Mode::Dynamic) => {
                // This arm is for when `cargo rustyrts dynamic` is called. We call `cargo build`,
                // but with the `RUSTC` env var set to the `cargo-rustyrts` binary so that we come back in the other branch,
                // and dispatch the invocations to `rustyrts-dynamic`, respectively.
                run_cargo_rustc_dynamic();
                select_and_execute_tests_dynamic();
            }
            _ => {
                show_error(
                    "`cargo-rustyrts` must be called with either `static` or `dynamic` as second argument."
                        .to_string(),
                )
            }
        }
    } else if let Some("rustc") = std::env::args().nth(1).as_ref().map(AsRef::as_ref) {
        // This arm is executed when `cargo-rustyrts` runs `cargo build` or `cargo test` with the `RUSTC_WRAPPER` env var set to itself.
        run_rustyrts();
    } else {
        show_error(
            "`cargo-rustyrts` must be called with either `rustyrts` or `rustc` as first argument."
                .to_string(),
        )
    }
}

//######################################################################################################################
// Actually important functions...

fn clean() {
    let dirs = if let Ok(dir) = std::env::var(ENV_TARGET_DIR) {
        vec![dir]
    } else {
        vec![
            format!("target_{}", Mode::Dynamic.to_string()),
            format!("target_{}", Mode::Static.to_string()),
        ]
    };

    for target_dir in dirs {
        let path_buf = PathBuf::from(target_dir);
        if path_buf.exists() {
            remove_dir_all(path_buf.clone()).expect(&format!(
                "Failed to remove directory {}",
                path_buf.display()
            ));
        }
    }
}

/// This will construct and execute a command like:
/// `rustyrts --crate-name some_crate_name --edition=2018 src/lib.rs --crate-type lib --domain interval`
fn run_rustyrts() {
    let mode_string = std::env::var(ENV_RUSTYRTS_MODE).unwrap_or("".to_string());
    let mode = FromStr::from_str(&mode_string).ok();

    let mut cmd = match mode {
        Some(Mode::Dynamic) => rustyrts_dynamic(),
        Some(Mode::Static) => rustyrts_static(),
        _ => panic!("Found unknown or unexpected mode"),
    };

    cmd.args(std::env::args().skip(2)); // skip `cargo rustc`

    // Add sysroot
    let sysroot = utils::compile_time_sysroot().expect("Cannot find sysroot");
    cmd.arg("--sysroot");
    cmd.arg(sysroot);

    // Add args for `rustyrts`
    let magic = std::env::var(ENV_RUSTYRTS_ARGS).expect(&format!("missing {}", ENV_RUSTYRTS_ARGS));
    let rustyrts_args: Vec<String> = serde_json::from_str(&magic)
        .expect(&format!("failed to deserialize {}", ENV_RUSTYRTS_ARGS));
    cmd.args(rustyrts_args);

    let verbose = std::env::var_os(ENV_RUSTYRTS_VERBOSE).is_some();
    if verbose {
        eprintln!("+ {:?}", cmd);
    }

    match cmd.status() {
        Ok(exit) => {
            if !exit.success() {
                std::process::exit(exit.code().unwrap_or(42));
            }
        }
        Err(ref e) => panic!("error during rustyrts run: {:?}", e),
    }
}

//######################################################################################################################
// STATIC RTS

/// This will construct and execute a command like:
/// `cargo build --bin some_crate_name -v -- --top_crate_name some_top_crate_name --domain interval -v cargo-rustyrts-marker-end`
/// using the rustc wrapper for static rustyrts
fn run_cargo_rustc_static() {
    let path_buf: PathBuf = get_static_path(false);

    create_dir_all(path_buf.as_path()).expect(&format!(
        "Failed to create directory {}",
        path_buf.display()
    ));

    let files = read_dir(path_buf.as_path()).unwrap();
    for path_res in files {
        if let Ok(path) = path_res {
            if path.file_name().to_str().unwrap().ends_with(ENDING_CHANGES) {
                remove_file(path.path()).unwrap();
            }
        }
    }

    let cmd = cargo_build(Mode::Static);
    execute(cmd);
}

/// This will construct and execute a command like:
/// * `cargo test --no-fail-fast -- --exact test_1 test_2 ...` (If some tests are affected)
/// * `cargo test --no-fail-fast --no-run` (If no tests are affected)
/// using the rustc wrapper for static rustyrts
fn select_and_execute_tests_static() {
    let verbose = has_arg_flag("-v");

    let path_buf = get_static_path(true);

    let files: Vec<DirEntry> = read_dir(path_buf.as_path())
        .unwrap()
        .map(|maybe_path| maybe_path.unwrap())
        .collect();

    // Read graphs
    let mut dependency_graph: DependencyGraph<String> = DependencyGraph::new();
    let edges = read_lines_filter_map(
        &files,
        ENDING_GRAPH,
        |line| !line.trim_start().starts_with("\\") && line.contains("\" -> \""),
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

        match file.write_all(format!("{}\n", dependency_graph.pretty(checksums_nodes)).as_bytes()) {
            Ok(_) => {}
            Err(reason) => panic!("Failed to write to file: {}", reason),
        };
    }

    // Read changed nodes
    let changed_nodes = read_lines_filter_map(
        &files,
        ENDING_CHANGES,
        |line| line != "" && dependency_graph.get_node(&line).is_some(),
        |line| dependency_graph.get_node(&line).unwrap(),
    );

    if verbose {
        println!(
            "Nodes that have changed:\n{}\n",
            changed_nodes.iter().sorted().join(", ")
        );
    } else {
        println!(
            "#Nodes that have changed: {}\n",
            changed_nodes.iter().count()
        );
    }

    // Read possibly affected tests
    let tests = read_lines_filter_map(
        &files,
        "test",
        |line| line != "" && dependency_graph.get_node(&line).is_some(),
        |line| dependency_graph.get_node(&line).unwrap(),
    );
    if verbose {
        println!(
            "Tests that have been found:\n{}\n",
            tests.iter().sorted().join(", ")
        );
    } else {
        println!("#Tests that have been found: {}\n", tests.iter().count());
    }

    #[cfg(not(feature = "print_paths"))]
    {
        let reached_nodes = dependency_graph.reachable_nodes(changed_nodes);
        let affected_tests: HashSet<&&String> = tests.intersection(&reached_nodes).collect();

        if verbose {
            println!(
                "Nodes that reach any changed node in the graph:\n{}\n",
                reached_nodes.iter().sorted().join(", ")
            );
        } else {
            println!(
                "#Nodes that reach any changed node in the graph: {}\n",
                reached_nodes.iter().count()
            );
        }

        if verbose {
            println!(
                "Affected tests:\n{}\n",
                affected_tests.iter().sorted().join(", ")
            );
        } else {
            println!("#Affected tests: {}\n", affected_tests.iter().count());
        }

        let cmd = cargo_test(Mode::Static, affected_tests.into_iter().map(|test| *test));

        execute(cmd);
    }
    #[cfg(feature = "print_paths")]
    {
        let (reached_nodes, affected_tests) = dependency_graph.affected_tests(changed_nodes, tests);

        if verbose {
            println!(
                "Nodes that reach any changed node in the graph:\n{}\n",
                reached_nodes.iter().sorted().join(", ")
            );
        } else {
            println!(
                "#Nodes that reach any changed node in the graph: {}\n",
                reached_nodes.iter().count()
            );
        }

        if verbose {
            println!(
                "Affected tests:\n{}\n",
                affected_tests
                    .iter()
                    .sorted()
                    .map(|(k, v)| format!("{}: [ {} ]", k, v.iter().join(" <- ")))
                    .join("\n")
            );
        } else {
            println!("#Affected tests: {}\n", affected_tests.iter().count());
        }

        let cmd = cargo_test(
            Mode::Static,
            affected_tests.keys().into_iter().map(|test| *test),
        );

        execute(cmd);
    }
}

//######################################################################################################################
// DYNAMIC RTS

/// This will construct and execute a command like:
/// `cargo build --bin some_crate_name -v -- cargo-rustyrts-marker-begin --top_crate_name some_top_crate_name --domain interval -v cargo-rustyrts-marker-end`
/// using the rustc wrapper for dynamic rustyrts
fn run_cargo_rustc_dynamic() {
    let path_buf: PathBuf = get_dynamic_path(false);

    create_dir_all(path_buf.as_path()).expect(&format!(
        "Failed to create directory {}",
        path_buf.display()
    ));

    let files = read_dir(path_buf.as_path()).unwrap();
    for path_res in files {
        if let Ok(path) = path_res {
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

    // Now we run `cargo build $FLAGS $ARGS`, giving the user the
    // chance to add additional arguments. `FLAGS` is set to identify
    // this target.  The user gets to control what gets actually passed to rustyrts.
    let cmd = cargo_build(Mode::Dynamic);
    execute(cmd);
}

// This will construct command line like:
// either   `cargo test --no-fail-fast -- --exact test_1 test_2 ...` (If some tests are affected)
// or       `cargo test --no-fail-fast --no-run` (If no tests are affected)
/// using the rustc wrapper for dynamic rustyrts
fn select_and_execute_tests_dynamic() {
    let verbose = has_arg_flag("-v");

    let path_buf = get_dynamic_path(true);

    let files: Vec<DirEntry> = read_dir(path_buf.as_path())
        .unwrap()
        .map(|maybe_path| maybe_path.unwrap())
        .collect();

    // Read tests
    let tests = read_lines(&files, ENDING_TEST);

    if verbose {
        println!(
            "Tests that have been found:\n{}\n",
            tests.iter().sorted().join(", ")
        );
    } else {
        println!("#Tests that have been found: {}\n", tests.iter().count());
    }

    // Read changed nodes
    let changed_nodes = read_lines(&files, ENDING_CHANGES);

    if verbose {
        println!(
            "Nodes that have changed:\n{}\n",
            changed_nodes.iter().sorted().join(", ")
        );
    } else {
        println!(
            "#Nodes that have changed: {}\n",
            changed_nodes.iter().count()
        );
    }

    // Read traces
    let mut affected_tests: Vec<String> = Vec::new();
    let traced_tests: Vec<&DirEntry> = files
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

    let traced_tests_names: HashSet<String> = traced_tests
        .iter()
        .map(|f| {
            f.file_name()
                .to_os_string()
                .into_string()
                .unwrap()
                .split_once(".")
                .unwrap()
                .0
                .to_string()
        })
        .collect();

    if verbose {
        println!(
            "Tests with traces:\n{}\n",
            traced_tests_names.iter().sorted().join(", ")
        );
    } else {
        println!(
            "#Tests with traces:: {}\n",
            traced_tests_names.iter().count()
        );
    }

    affected_tests.append(
        &mut tests
            .difference(&traced_tests_names)
            .map(|s| s.clone())
            .collect_vec(),
    );

    for file in traced_tests {
        let traced_nodes: HashSet<String> = read_to_string(file.path())
            .unwrap()
            .split("\n")
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();

        let intersection: HashSet<&String> = traced_nodes.intersection(&changed_nodes).collect();
        if !intersection.is_empty() {
            let test_name = file
                .file_name()
                .into_string()
                .unwrap()
                .split_once(".")
                .unwrap()
                .0
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
        println!("#Affected tests: {}\n", affected_tests.iter().count());
    }

    let cmd = cargo_test(Mode::Dynamic, affected_tests.iter());
    execute(cmd);
}
