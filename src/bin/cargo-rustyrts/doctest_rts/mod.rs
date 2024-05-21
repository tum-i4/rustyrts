use std::{collections::HashSet, ffi::OsString, path::PathBuf};

use cargo::{
    core::{
        compiler::{Compilation, CompileKind, Doctest},
        Verbosity, Workspace,
    },
    ops::CompileOptions,
    util::add_path_args,
    CliError,
};
use itertools::Itertools;
use rustyrts::constants::{ENV_DOCTESTED, ENV_TARGET_DIR};

/// Runs doc tests.
///
/// Returns a `Vec` of tests that failed when `--no-fail-fast` is used.
/// If `--no-fail-fast` is *not* used, then this returns an `Err`.
pub(crate) fn run_analysis_doctests(
    ws: &Workspace<'_>,
    test_args: &[String],
    compilation: &Compilation<'_>,
) -> Result<HashSet<String>, CliError> {
    let mut test_names: HashSet<String> = HashSet::new();

    let config = ws.config();

    for doctest_info in &compilation.to_doc_test {
        let Doctest {
            args,
            unstable_opts,
            unit,
            linker: _,
            script_meta,
            env,
        } = doctest_info;

        let p = {
            let mut args = args.to_owned();

            let mut rlib_source =
                PathBuf::from(std::env::var("CARGO_HOME").expect("Did not find CARGO_HOME"));
            rlib_source.push("bin");
            args.push("-L".into());
            args.push(rlib_source.into_os_string());

            config
                .shell()
                .status("Analyzing Doc-tests", unit.target.name())?;
            let mut p = compilation.rustdoc_process(unit, *script_meta)?;

            for (var, value) in env {
                p.env(var, value);
            }

            p.arg("--crate-name").arg(&unit.target.crate_name());
            p.arg("--test");

            add_path_args(ws, unit, &mut p);
            p.arg("--test-run-directory").arg(unit.pkg.root());

            if let CompileKind::Target(target) = unit.kind {
                // use `rustc_target()` to properly handle JSON target paths
                p.arg("--target").arg(target.rustc_target());
            }

            for &rust_dep in &[
                &compilation.deps_output[&unit.kind],
                &compilation.deps_output[&CompileKind::Host],
            ] {
                let mut arg = OsString::from("dependency=");
                arg.push(rust_dep);
                p.arg("-L").arg(arg);
            }

            for native_dep in compilation.native_dirs.iter() {
                p.arg("-L").arg(native_dep);
            }

            for arg in test_args {
                p.arg("--test-args").arg(arg);
            }

            if config.shell().verbosity() == Verbosity::Quiet {
                p.arg("--test-args").arg("--quiet");
            }

            p.args(unit.pkg.manifest().lint_rustflags());

            p.args(&args);

            if *unstable_opts {
                p.arg("-Zunstable-options");
            }

            if config.extra_verbose() {
                p.display_env_vars();
            }

            config
                .shell()
                .verbose(|shell| shell.status("Running", p.to_string()))?;

            p
        };

        {
            let mut p = p.clone();
            p.arg("--test-args");
            p.arg("--list");
            let output = p.exec_with_output()?;
            let stdout = std::str::from_utf8(&output.stdout).unwrap();
            let tests = parse_tests(stdout);
            test_names.extend(tests);
        }

        {
            let mut p = p.clone();

            let rustc_wrapper = {
                let mut path_buf =
                    std::env::current_exe().expect("current executable path invalid");
                path_buf.set_file_name("rustyrts-static-doctest");
                path_buf
            };
            p.arg("-Z");
            p.arg("unstable-options");
            p.arg("--test-builder");
            p.arg(rustc_wrapper);
            p.env(ENV_TARGET_DIR, ws.target_dir().into_path_unlocked());
            p.env(ENV_DOCTESTED, unit.target.name());

            p.arg("--runtool");
            p.arg("echo"); // Just a dummy command

            p.arg("--nocapture");

            // let _ = p.output();
            let _ = p.output();
        }
    }

    Ok(test_names)
}

fn parse_tests(input: &str) -> Vec<String> {
    input
        .lines()
        .filter_map(|l| l.strip_suffix(": test"))
        .map(|s| s.to_string())
        .collect_vec()
}
