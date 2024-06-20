#![allow(dead_code)]

use anyhow::format_err;
use cargo::core::{TargetKind, Workspace};
use cargo::ops;
use cargo::{
    core::compiler::{BuildContext, Context},
    util::errors::CargoResult,
};
use cargo::{
    core::compiler::{Compilation, CompileKind, Doctest, Metadata, Unit, UnitInterner, UnitOutput},
    ops::create_bcx,
};
use cargo::{
    core::{
        compiler::{
            unit_graph::{self},
            Executor,
        },
        shell::Verbosity,
    },
    util::profile,
};
use cargo::{
    ops::CompileOptions,
    util::{add_path_args, CliError, CliResult, Config},
};
use cargo_util::{ProcessBuilder, ProcessError};
use internment::Arena;
use itertools::Itertools;
use rustyrts::constants::ENV_TARGET_DIR;
use std::{ffi::OsString, sync::Arc};
use std::{fmt::Write, time::Instant};
use std::{
    path::{Path, PathBuf},
    string::String,
};
use tracing::trace;

use crate::{
    commands::{SelectionMode, Selector, TestUnit},
    doctest_rts::run_analysis_doctests,
};

//#####################################################################################################################
// Source: https://github.com/rust-lang/cargo/blob/d0390c22b16ea6c800754fb7620ab8ee31debcc7/src/cargo/ops/cargo_test.rs
// Adapted into selecting tests before executing them
//#####################################################################################################################

/// The kind of test.
///
/// This is needed because `Unit` does not track whether or not something is a
/// benchmark.
#[derive(Copy, Clone)]
enum TestKind {
    Test,
    Bench,
    Doctest,
}

/// A unit that failed to run.
struct UnitTestError {
    unit: Unit,
    kind: TestKind,
}

impl UnitTestError {
    /// Returns the CLI args needed to target this unit.
    fn cli_args(&self, ws: &Workspace<'_>, opts: &ops::CompileOptions) -> String {
        let mut args = if opts.spec.needs_spec_flag(ws) {
            format!("-p {} ", self.unit.pkg.name())
        } else {
            String::new()
        };
        let mut add = |which| write!(args, "--{which} {}", self.unit.target.name()).unwrap();

        match self.kind {
            TestKind::Test | TestKind::Bench => match self.unit.target.kind() {
                TargetKind::Lib(_) => args.push_str("--lib"),
                TargetKind::Bin => add("bin"),
                TargetKind::Test => add("test"),
                TargetKind::Bench => add("bench"),
                TargetKind::ExampleLib(_) | TargetKind::ExampleBin => add("example"),
                TargetKind::CustomBuild => panic!("unexpected CustomBuild kind"),
            },
            TestKind::Doctest => args.push_str("--doc"),
        }
        args
    }
}

/// Compiles and runs tests.
///
/// On error, the returned [`CliError`] will have the appropriate process exit
/// code that Cargo should use.
pub fn run_tests(
    ws: &Workspace<'_>,
    options: &CompileOptions,
    test_args: &[&str],
    mode: &dyn SelectionMode,
) -> CliResult {
    let target_dir = ws.target_dir().into_path_unlocked();

    let interner = UnitInterner::new();
    let bcx = create_bcx(ws, options, &interner)?;
    let unit_graph = &bcx.unit_graph;

    mode.prepare_cache(&target_dir, unit_graph);
    mode.prepare_cache(&target_dir, unit_graph);

    let exec: Arc<dyn Executor> = Arc::new(mode.executor(target_dir.clone()));
    let compilation = compile_tests(ws, options, &bcx, exec)?;

    let arena = Arena::new();
    let mut selection_context = mode.selection_context(ws, &target_dir, &arena, unit_graph);
    let selector = selection_context.selector();

    let mut errors: Vec<UnitTestError> = run_unit_tests(
        ws,
        options,
        test_args,
        &compilation,
        TestKind::Test,
        selector,
        &arena,
        &target_dir,
    )?;

    let doctest_errors =
        run_doc_tests(ws, options, test_args, &compilation, selector, &target_dir)?;

    errors.extend(doctest_errors);

    mode.clean_cache(&target_dir);
    mode.clean_cache(&target_dir);

    no_fail_fast_err(ws, options, &errors)
}

/// Runs the unit and integration tests of a package.
///
/// Returns a `Vec` of tests that failed when `--no-fail-fast` is used.
/// If `--no-fail-fast` is *not* used, then this returns an `Err`.
fn run_unit_tests<'compilation, 'context, 'arena: 'context>(
    ws: &Workspace<'_>,
    options: &CompileOptions,
    test_args: &[&str],
    compilation: &'context Compilation<'compilation>,
    test_kind: TestKind,
    selector: &mut dyn Selector<'context>,
    arena: &'arena Arena<String>,
    target_dir: &Path,
) -> Result<Vec<UnitTestError>, CliError> {
    let config = ws.config();
    let cwd = config.cwd();
    let mut errors = Vec::new();

    selector.note(&mut config.shell(), test_args);

    let mut test_args = Vec::from(test_args);
    if test_args.iter().any(|s| s == &"--exact") {
        panic!("Regression Test Selection is incompatible to using --exact");
    } else {
        test_args.push("--exact");
    }

    for UnitOutput {
        unit,
        path,
        script_meta,
    } in &compilation.tests
    {
        let start_time = Instant::now();
        let test_unit = TestUnit::test(unit, arena, target_dir);
        let affected_tests = selector.select_tests(test_unit, &mut config.shell(), start_time);

        let prefix = unit.target.crate_name().to_string() + "::";
        trace!("Stripping crate name {:?}", prefix);
        let mut affected_tests = affected_tests
            .iter()
            .filter_map(|s| s.strip_prefix(&prefix))
            .collect_vec();

        let mut test_args = test_args.clone();
        if affected_tests.is_empty() {
            test_args.push("?"); // This excludes all tests
        } else {
            test_args.append(&mut affected_tests);
        }

        let (exe_display, mut cmd) = cmd_builds(
            config,
            cwd,
            unit,
            path,
            script_meta,
            &test_args,
            compilation,
            "unittests",
        )?;

        cmd.env(ENV_TARGET_DIR, ws.target_dir().into_path_unlocked());

        if config.extra_verbose() {
            cmd.display_env_vars();
        }

        config
            .shell()
            .concise(|shell| shell.status("Running", &exe_display))?;
        config
            .shell()
            .verbose(|shell| shell.status("Running", &cmd))?;

        if let Err(e) = cmd.exec() {
            let unit_err = UnitTestError {
                unit: unit.clone(),
                kind: test_kind,
            };
            report_test_error(ws, &test_args, options, &unit_err, e);
            errors.push(unit_err);
        }
    }
    Ok(errors)
}

/// Runs doc tests.
///
/// Returns a `Vec` of tests that failed when `--no-fail-fast` is used.
/// If `--no-fail-fast` is *not* used, then this returns an `Err`.
fn run_doc_tests<'compilation, 'context, 'arena: 'context>(
    ws: &Workspace<'_>,
    options: &CompileOptions,
    test_args: &[&str],
    compilation: &'context Compilation<'compilation>,
    selector: &mut dyn Selector<'context>,
    target_dir: &Path,
) -> Result<Vec<UnitTestError>, CliError> {
    let config = ws.config();
    let mut errors = Vec::new();

    // ISSUE: rustdoc splits arguments on whitespaces
    // Since all names of doctests include spaces, we cannot use --exact here
    //
    // if !test_args.iter().any(|s| s == "--exact") {
    //     test_args.push("--exact".to_string());
    // }

    for doctest_info in &compilation.to_doc_test {
        let Doctest {
            args,
            unstable_opts,
            unit,
            linker: _,
            script_meta,
            env,
        } = doctest_info;

        let mut args = args.to_owned();

        let mut rlib_source =
            PathBuf::from(std::env::var("CARGO_HOME").expect("Did not find CARGO_HOME"));
        rlib_source.push("bin");
        args.push("-L".into());
        args.push(rlib_source.into_os_string());

        let start = Instant::now();
        let tests = run_analysis_doctests(
            ws,
            test_args,
            compilation,
            target_dir,
            doctest_info,
            selector.cache_kind(),
            selector.doctest_callback_analysis(),
        )?;
        let test_unit = TestUnit::doctest(unit, tests);
        let affected_tests = selector.select_tests(test_unit, &mut config.shell(), start);

        let mut test_args = Vec::from(test_args);
        let mut affected_tests = affected_tests.iter().map(String::as_str).collect_vec();

        if affected_tests.is_empty() {
            test_args.push("?"); // This excludes all tests
        } else {
            test_args.append(&mut affected_tests);
        }

        config.shell().status("Doc-tests", unit.target.name())?;
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

        for native_dep in &compilation.native_dirs {
            p.arg("-L").arg(native_dep);
        }

        for arg in &test_args {
            p.arg("--test-args").arg(arg);
        }

        if config.shell().verbosity() == Verbosity::Quiet {
            p.arg("--test-args").arg("--quiet");
        }

        p.args(unit.pkg.manifest().lint_rustflags());

        p.args(&args);

        let callback = selector.doctest_callback_execution();
        callback(&mut p, target_dir, unit);

        if *unstable_opts {
            p.arg("-Zunstable-options");
        }

        if config.extra_verbose() {
            p.display_env_vars();
        }

        config
            .shell()
            .verbose(|shell| shell.status("Running", p.to_string()))?;

        if let Err(e) = p.exec() {
            let unit_err = UnitTestError {
                unit: unit.clone(),
                kind: TestKind::Doctest,
            };
            report_test_error(ws, &test_args, options, &unit_err, e);
            errors.push(unit_err);
        }
    }
    Ok(errors)
}

/// Creates a [`ProcessBuilder`] for executing a single test.
///
/// Returns a tuple `(exe_display, process)` where `exe_display` is a string
/// to display that describes the executable path in a human-readable form.
/// `process` is the `ProcessBuilder` to use for executing the test.
fn cmd_builds(
    config: &Config,
    cwd: &Path,
    unit: &Unit,
    path: &PathBuf,
    script_meta: &Option<Metadata>,
    test_args: &[&str],
    compilation: &Compilation<'_>,
    exec_type: &str,
) -> CargoResult<(String, ProcessBuilder)> {
    let test_path = unit.target.src_path().path().unwrap();
    let short_test_path = test_path
        .strip_prefix(unit.pkg.root())
        .unwrap_or(test_path)
        .display();

    let exe_display = match unit.target.kind() {
        TargetKind::Test | TargetKind::Bench => format!(
            "{} ({})",
            short_test_path,
            path.strip_prefix(cwd).unwrap_or(path).display()
        ),
        _ => format!(
            "{} {} ({})",
            exec_type,
            short_test_path,
            path.strip_prefix(cwd).unwrap_or(path).display()
        ),
    };

    let mut cmd = compilation.target_process(path, unit.kind, &unit.pkg, *script_meta)?;
    cmd.args(test_args);
    if unit.target.harness() && config.shell().verbosity() == Verbosity::Quiet {
        cmd.arg("--quiet");
    }

    Ok((exe_display, cmd))
}

/// Returns the `CliError` when using `--no-fail-fast` and there is at least
/// one error.
fn no_fail_fast_err(
    ws: &Workspace<'_>,
    opts: &ops::CompileOptions,
    errors: &[UnitTestError],
) -> CliResult {
    let args: Vec<_> = errors
        .iter()
        .map(|unit_err| format!("    `{}`", unit_err.cli_args(ws, opts)))
        .collect();
    let message = match errors.len() {
        0 => return Ok(()),
        1 => format!("1 target failed:\n{}", args.join("\n")),
        n => format!("{n} targets failed:\n{}", args.join("\n")),
    };
    Err(anyhow::Error::msg(message).into())
}

/// Displays an error on the console about a test failure.
fn report_test_error(
    ws: &Workspace<'_>,
    test_args: &[&str],
    opts: &ops::CompileOptions,
    unit_err: &UnitTestError,
    test_error: anyhow::Error,
) {
    let which = match unit_err.kind {
        TestKind::Test => "test failed",
        TestKind::Bench => "bench failed",
        TestKind::Doctest => "doctest failed",
    };

    let mut err = format_err!("{}, to rerun pass `{}`", which, unit_err.cli_args(ws, opts));
    // Don't show "process didn't exit successfully" for simple errors.
    // libtest exits with 101 for normal errors.
    let (is_simple, executed) = test_error
        .downcast_ref::<ProcessError>()
        .and_then(|proc_err| proc_err.code)
        .map_or((false, false), |code| (code == 101, true));

    if !is_simple {
        err = test_error.context(err);
    }

    cargo::display_error(&err, &mut ws.config().shell());

    let harness: bool = unit_err.unit.target.harness();
    let nocapture: bool = test_args.iter().any(|s| s == &"--nocapture");

    if !is_simple && executed && harness && !nocapture {
        drop(ws.config().shell().note(
            "test exited abnormally; to see the full output pass --nocapture to the harness.",
        ));
    }
}

//#####################################################################################################################
// Source: https://github.com/rust-lang/cargo/blob/d0390c22b16ea6c800754fb7620ab8ee31debcc7/src/cargo/ops/cargo_compile/mod.rs
// Adapted to use custom executor and re-use existing context
//#####################################################################################################################

fn compile_tests<'a>(
    ws: &Workspace<'a>,
    options: &CompileOptions,
    bcx: &BuildContext<'a, 'a>,
    exec: Arc<dyn Executor>,
) -> CargoResult<Compilation<'a>> {
    let mut compilation = compile_with_exec(ws, options, bcx, &exec)?;
    compilation.tests.sort();

    Ok(compilation)
}

fn compile_with_exec<'a>(
    ws: &Workspace<'a>,
    options: &CompileOptions,
    bcx: &BuildContext<'a, 'a>,
    exec: &Arc<dyn Executor>,
) -> CargoResult<Compilation<'a>> {
    ws.emit_warnings()?;
    compile_ws(ws, options, bcx, exec)
}

fn compile_ws<'a>(
    ws: &Workspace<'a>,
    options: &CompileOptions,
    bcx: &BuildContext<'a, 'a>,
    exec: &Arc<dyn Executor>,
) -> CargoResult<Compilation<'a>> {
    if options.build_config.unit_graph {
        unit_graph::emit_serialized_unit_graph(&bcx.roots, &bcx.unit_graph, ws.config())?;
        return Compilation::new(bcx);
    }
    cargo::core::gc::auto_gc(bcx.config);
    let _p = profile::start("compiling");
    let cx = Context::new(bcx)?;
    cx.compile(exec)
}
