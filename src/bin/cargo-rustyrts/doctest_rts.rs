use std::{
    collections::{HashMap, HashSet},
    ffi::OsString,
    hash::DefaultHasher,
    hash::Hasher,
    path::{Path, PathBuf},
    string::ToString,
};

use cargo::{
    core::{
        compiler::{Compilation, CompileKind, Doctest, Unit},
        Verbosity, Workspace,
    },
    util::add_path_args,
    CliError,
};
use cargo_util::ProcessBuilder;
use itertools::Itertools;
use rustyrts::{
    callbacks_shared::{ChecksumsCallback, CompileMode, RTSContext, Target},
    checksums::Checksums,
    fs_utils::{CacheKind, ChecksumKind},
};

use crate::commands::DoctestName;

struct DoctestAnalysis {
    path: PathBuf,
    context: RTSContext,
}

impl ChecksumsCallback for DoctestAnalysis {
    fn path(&self) -> &Path {
        &self.path
    }

    fn context(&self) -> &RTSContext {
        &self.context
    }

    fn context_mut(&mut self) -> &mut RTSContext {
        &mut self.context
    }
}

/// Analyzes doc tests.
///
/// Returns a `Vec` of tests that have been found.
pub(crate) fn run_analysis_doctests(
    ws: &Workspace<'_>,
    test_args: &[&str],
    compilation: &Compilation<'_>,
    target_dir: &Path,
    doctest_info: &Doctest,
    cache_kind: CacheKind,
    callback: impl FnOnce(&mut ProcessBuilder, &Path, &Unit),
) -> Result<HashSet<String>, CliError> {
    let mut test_names: HashSet<String> = HashSet::new();

    let config = ws.config();

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
        if let Ok(output) = p.output() {
            let stdout = std::str::from_utf8(&output.stdout).unwrap();
            let tests = parse_tests(stdout);
            test_names.extend(tests);
        }
    }

    {
        let grouped = {
            let mut groups = HashMap::new();

            for test in &test_names {
                let doctest_name = DoctestName::new(test.clone());
                let key = (doctest_name.fn_name(), doctest_name.cache_name());

                if groups.get(&key).is_none() {
                    groups.insert(key.clone(), Vec::new()).unwrap_or_default();
                }
                groups.get_mut(&key).unwrap().push(doctest_name);
            }

            groups
        };

        let mut tests = Vec::new();

        for ((fn_name, cache_name), names) in grouped {
            let compile_mode = CompileMode::try_from(format!("{:?}", unit.mode).as_str()).unwrap();
            let target =
                Target::try_from(format!("{}", unit.target.kind().description()).as_str()).unwrap();

            let mut analysis = DoctestAnalysis {
                path: target_dir.to_path_buf(),
                context: RTSContext::new(
                    unit.target.crate_name(),
                    compile_mode,
                    target,
                    Some(cache_name.clone()),
                    Some(fn_name.clone()),
                ),
            };

            let old_checksums = analysis.import_checksums(ChecksumKind::Checksum, true);
            let old_checksums_vtbl = analysis.import_checksums(ChecksumKind::VtblChecksum, true);
            let old_checksums_const = analysis.import_checksums(ChecksumKind::ConstChecksum, true);

            {
                let context = &mut analysis.context;

                context.old_checksums.get_or_init(|| old_checksums);
                context
                    .old_checksums_vtbl
                    .get_or_init(|| old_checksums_vtbl);
                context
                    .old_checksums_const
                    .get_or_init(|| old_checksums_const);
            }

            tests.push((fn_name, analysis, names));
        }

        {
            let mut p = p.clone();
            callback(&mut p, target_dir, unit);

            for arg in test_args {
                p.arg("--test-args").arg(arg);
            }

            let _ = p.output();
        }

        for (fn_name, analysis, names) in &mut tests {
            let mut checksums = Checksums::new();

            // We add a hash of the names of all tests, to recognize newly added or removed compile-fail tests
            let hash = {
                let mut hasher = DefaultHasher::new();
                for name in names {
                    hasher.write(name.full_name().as_bytes());
                }
                let value = hasher.finish();

                (value, value)
            };

            if checksums.get(fn_name).is_none() {
                checksums
                    .insert(fn_name.clone(), HashSet::new())
                    .unwrap_or_default();
            }
            checksums.get_mut(fn_name).unwrap().insert(hash);

            analysis.export_checksums(ChecksumKind::Checksum, &checksums, true);
        }

        for (_fn_name, analysis, _count) in &mut tests {
            let checksums = analysis.import_checksums(ChecksumKind::Checksum, false);
            let checksums_vtbl = analysis.import_checksums(ChecksumKind::VtblChecksum, false);
            let checksums_const = analysis.import_checksums(ChecksumKind::ConstChecksum, false);

            let RTSContext {
                new_checksums,
                new_checksums_vtbl,
                new_checksums_const,
                ..
            } = analysis.context_mut();

            new_checksums.get_or_init(|| checksums);
            new_checksums_vtbl.get_or_init(|| checksums_vtbl);
            new_checksums_const.get_or_init(|| checksums_const);

            analysis.export_changes(cache_kind);
        }
    }

    Ok(test_names)
}

fn parse_tests(input: &str) -> Vec<String> {
    input
        .lines()
        .filter_map(|l| l.strip_suffix(": test"))
        .map(ToString::to_string)
        .collect_vec()
}
