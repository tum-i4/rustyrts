use itertools::Itertools;
use rustyrts::graph::graph::DependencyGraph;
use rustyrts::paths::get_base_path;
use rustyrts::utils;
use serde_json;
use std::collections::HashSet;
use std::ffi::OsString;
use std::fs::{create_dir_all, read_dir, read_to_string, remove_file, DirEntry, OpenOptions};
use std::io::Write;
use std::process::Command;

//######################################################################################################################
// This file is heavily inspired by rust-mir-checker
// Source: https://github.com/lizhuohua/rust-mir-checker/blob/86c3c26e797d3e25a38044fa98b765c5d220e4ea/src/bin/cargo-mir-checker.rs
//######################################################################################################################

const CARGO_RUSTYRTS_HELP: &str = r#"Static regression test selection based on the MIR

Usage:
    cargo rustyrts
"#;

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

fn rustyrts() -> Command {
    let mut path = std::env::current_exe().expect("current executable path invalid");
    path.set_file_name("rustyrts");
    Command::new(path)
}

fn cargo() -> Command {
    Command::new(std::env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo")))
}

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
        // This arm is for when `cargo rustyrts` is called. We call `cargo rustc` for each applicable target,
        // but with the `RUSTC` env var set to the `cargo-rustyrts` binary so that we come back in the other branch,
        // and dispatch the invocations to `rustc` and `rustyrts`, respectively.
        run_cargo_rustc();
        select_and_execute_tests();
    } else if let Some("rustc") = std::env::args().nth(1).as_ref().map(AsRef::as_ref) {
        // This arm is executed when `cargo-rustyrts` runs `cargo rustc` with the `RUSTC_WRAPPER` env var set to itself.
        run_rustyrts();
    } else {
        show_error(
            "`cargo-rustyrts` must be called with either `rustyrts` or `rustc` as first argument."
                .to_string(),
        )
    }
}

// This will construct command line like:
// `cargo rustc --bin some_crate_name -v -- cargo-rustyrts-marker-begin --top_crate_name some_top_crate_name --domain interval -v cargo-rustyrts-marker-end`
// And set the following environment variables:
// `RUSTC_WRAPPER` is set to `cargo-rustyrts` itself so the execution will come back to the second branch as described above
// `rustyrts_args` is set to the user-provided arguments for `rustyrts`
// `RUSTYRTS_VERBOSE` is set if `-v` is provided
fn run_cargo_rustc() {
    let verbose = has_arg_flag("-v");

    let project_dir = std::env::current_dir().unwrap();
    let path_buf = get_base_path(project_dir.to_str().unwrap());
    create_dir_all(path_buf.as_path()).expect("Failed to create parent directories");

    let files = read_dir(path_buf.as_path()).unwrap();
    for path_res in files {
        if let Ok(path) = path_res {
            if path.file_name().to_str().unwrap().ends_with("changes") {
                remove_file(path.path()).unwrap();
            }
        }
    }

    // Now run the command.

    let mut args = std::env::args().skip(2);

    // Now we run `cargo rustc $FLAGS $ARGS`, giving the user the
    // chance to add additional arguments. `FLAGS` is set to identify
    // this target.  The user gets to control what gets actually passed to rustyrts.
    let mut cmd = cargo();
    cmd.arg("rustc");
    cmd.arg("--profile");
    cmd.arg("test");

    // Add cargo args until first `--`.
    while let Some(arg) = args.next() {
        if arg == "--" {
            break;
        }
        cmd.arg(arg);
    }

    // Store directory of the project, such that rustyrts knows where to store information about tests, changes
    // and the graph
    cmd.env("PROJECT_DIR", project_dir.to_str().unwrap());

    // Serialize the remaining args into a special environemt variable.
    // This will be read by `inside_cargo_rustc` when we go to invoke
    // our actual target crate.
    let args_vec: Vec<String> = args.collect();
    cmd.env(
        "rustyrts_args",
        serde_json::to_string(&args_vec).expect("failed to serialize args"),
    );

    // Replace the rustc executable through RUSTC_WRAPPER environment variable
    let path = std::env::current_exe().expect("current executable path invalid");
    cmd.env("RUSTC_WRAPPER", path);

    if verbose {
        cmd.env("RUSTYRTS_VERBOSE", ""); // this makes `inside_cargo_rustc` verbose.
        eprintln!("+ {:?}", cmd);
    }

    // Execute cmd
    let exit_status = cmd
        .spawn()
        .expect("could not run cargo")
        .wait()
        .expect("failed to wait for cargo?");

    if !exit_status.success() {
        std::process::exit(exit_status.code().unwrap_or(-1))
    }
}

// This will construct command line like:
// `rustyrts --crate-name some_crate_name --edition=2018 src/lib.rs --crate-type lib --domain interval`
fn run_rustyrts() {
    let mut cmd = rustyrts();
    cmd.args(std::env::args().skip(2)); // skip `cargo rustc`

    // Add sysroot
    let sysroot = utils::compile_time_sysroot().expect("Cannot find sysroot");
    cmd.arg("--sysroot");
    cmd.arg(sysroot);

    // Add args for `rustyrts`
    let magic = std::env::var("rustyrts_args").expect("missing rustyrts_args");
    let rustyrts_args: Vec<String> =
        serde_json::from_str(&magic).expect("failed to deserialize rustyrts_args");
    cmd.args(rustyrts_args);

    //let verbose = std::env::var_os("RUSTYRTS_VERBOSE").is_some();
    //if verbose {
    //    eprintln!("+ {:?}", cmd);
    //}

    match cmd.status() {
        Ok(exit) => {
            if !exit.success() {
                std::process::exit(exit.code().unwrap_or(42));
            }
        }
        Err(ref e) => panic!("error during rustyrts run: {:?}", e),
    }
}

fn select_and_execute_tests() {
    let verbose = has_arg_flag("-v");

    let mut cmd = cargo();
    cmd.arg("test");
    cmd.arg("--no-fail-fast"); // Do not stop if a test fails, execute all included tests

    let path_buf = get_base_path(std::env::current_dir().unwrap().to_str().unwrap());

    let files: Vec<DirEntry> = read_dir(path_buf.as_path())
        .unwrap()
        .map(|maybe_path| maybe_path.unwrap())
        .collect();

    // Read possibly affected tests
    let tests = read_lines(&files, "test", |line| line != "");
    if verbose {
        println!("Tests that have been found:\n{}\n", tests.iter().join("\n"));
    }

    // Read changed nodes
    let changed_nodes: HashSet<String> = read_lines(&files, "changes", |line| line != "");
    if verbose {
        println!(
            "Nodes that have changed:\n{}\n",
            changed_nodes.iter().join("\n")
        );
    }

    // Read graphs
    let mut dependency_graph: DependencyGraph<String> = DependencyGraph::new();
    let edges = read_lines(&files, "dot", |line| line.contains(" -> "));
    dependency_graph.import_edges(edges);

    if verbose {
        let mut complete_graph_path = path_buf.clone();
        complete_graph_path.push("!complete_graph.dot");
        let mut file = match OpenOptions::new()
            .create(true)
            .write(true)
            .append(false)
            .open(complete_graph_path.as_path())
        {
            Ok(file) => file,
            Err(reason) => panic!("Failed to open file: {}", reason),
        };

        match file.write_all(format!("{}\n", dependency_graph.to_string()).as_bytes()) {
            Ok(_) => {}
            Err(reason) => panic!("Failed to write to file: {}", reason),
        };
    }

    let reached_nodes = dependency_graph.reachable_nodes(changed_nodes);
    if verbose {
        println!(
            "Nodes that reach any changed node in the graph:\n{}\n",
            reached_nodes.iter().join("\n")
        );
    }

    let affected_tests: HashSet<&String> = tests.intersection(&reached_nodes).collect();
    if verbose {
        println!("Affected tests:\n{}\n", affected_tests.iter().join("\n"));
    }

    let mut affected_tests_iter = affected_tests
        .iter()
        .map(|line| {
            let (_, test) = line.split_once("::").unwrap();
            test.to_string()
        })
        .peekable();
    if let None = affected_tests_iter.peek() {
        cmd.arg("--no-run");
    } else {
        cmd.arg("--");
        cmd.arg("--exact");
        for test in affected_tests_iter {
            cmd.arg(test);
        }
    }

    if verbose {
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

fn read_lines<F>(files: &Vec<DirEntry>, file_ending: &str, filter: F) -> HashSet<String>
where
    F: Fn(&String) -> bool,
{
    let tests: HashSet<String> = files
        .iter()
        .filter(|path| path.file_name().to_str().unwrap().ends_with(file_ending))
        .flat_map(|path| {
            let content = read_to_string(path.path()).unwrap();
            let lines: Vec<String> = content.split("\n").map(|s| s.to_string()).collect();
            lines
        })
        .filter(filter)
        .collect();
    tests
}
