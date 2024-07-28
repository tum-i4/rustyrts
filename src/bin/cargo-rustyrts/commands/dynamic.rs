use std::{
    collections::{HashMap, HashSet},
    fs::{read_dir, read_to_string},
    path::{Path, PathBuf},
    string::ToString,
    sync::Arc,
    time::Instant,
    vec::Vec,
};

use cargo::{
    core::{compiler::Executor, Workspace},
    util::command_prelude::*,
};
use cargo::{
    core::{
        compiler::{
            unit_graph::{UnitDep, UnitGraph},
            Unit,
        },
        Shell,
    },
    CargoResult,
};
use cargo_util::ProcessBuilder;
use internment::{Arena, ArenaIntern};

use itertools::Itertools;
use rustyrts::{
    callbacks_shared::DOCTEST_PREFIX,
    constants::{
        ENDING_CHANGES, ENDING_PROCESS_TRACE, ENV_COMPILE_MODE, ENV_DOCTESTED,
        ENV_ONLY_INSTRUMENTATION, ENV_TARGET_DIR,
    },
    fs_utils::{CacheFileDescr, CacheFileKind, CacheKind},
};
use test::{test::parse_opts, TestOpts};

use crate::{
    commands::{convert_doctest_name, TestInfo},
    ops::PreciseExecutor,
};

use super::{
    cache::HashCache, DependencyUnit, PreciseSelectionMode, Selection, SelectionContext,
    SelectionMode, SelectionUnit, Selector, TestUnit,
};

pub fn cli() -> Command {
    subcommand("dynamic")
        .about(r"Perform regression test selection using a dynamic technique, collecting runtime traces

 +++ extremely precise
 + can trace child processes (linux only)
 - prone to flakiness in case of random test input
 - tampers with binaries (not always desired)
 - needs to isolate tests in separate processes if tests are executed in parallel (not always feasible)
 - needs to execute test sequentially/single-threaded on Windows
 / small compilation overhead, moderate runtime overhead

Consider using `cargo rustyrts static` instead if your tests rely on random input.")
        .arg(
            Arg::new("args")
                .value_name("ARGS")
                .help("Arguments for the test binary")
                .num_args(0..)
                .last(true),
        )
        .arg(flag("doc", "Test only this library's documentation"))
        .arg(flag("no-run", "Compile, but don't run tests"))
        .arg_ignore_rust_version()
        .arg_future_incompat_report()
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
        .arg_targets_all(
            "Test only this package's library",
            "Test only the specified binary",
            "Test all binaries",
            "Test only the specified example",
            "Test all examples",
            "Test only the specified test target",
            "Test all test targets",
            "Test only the specified bench target",
            "Test all bench targets",
            "Test all targets (does not include doctests)",
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
    fn default_target_dir(&self, target_dir: PathBuf) -> std::path::PathBuf {
        let mut target_dir = target_dir;
        target_dir.push("dynamic");
        target_dir
    }

    fn executor(&self, target_dir: PathBuf) -> Arc<dyn Executor> {
        let mut path_buf = std::env::current_exe().expect("current executable path invalid");
        path_buf.set_file_name("rustyrts-dynamic");

        Arc::new(PreciseExecutor::new(path_buf, target_dir))
    }
}

impl PreciseSelectionMode for DynamicMode {
    fn prepare_cache(&self, target_dir: &std::path::Path, unit_graph: &UnitGraph) {
        for kind in [CacheKind::General, CacheKind::Dynamic] {
            let path = kind.map(target_dir.to_path_buf());
            std::fs::create_dir_all(path).expect("Failed to create cache directory");
        }

        for unit in unit_graph.keys() {
            match unit.mode {
                CompileMode::Test
                | CompileMode::Build
                | CompileMode::Doctest
                | CompileMode::RunCustomBuild => {}
                mode => panic!("Found unexpected compile mode, {mode:?}"),
            }
        }
    }

    fn clean_cache(&self, target_dir: &Path) {
        let path = CacheKind::Dynamic.map(target_dir.to_path_buf());
        if let Ok(files) = read_dir(path) {
            for dir_entry in files.flatten() {
                let file_name = dir_entry.file_name();
                let file_name = file_name.to_str().unwrap();

                if file_name.ends_with(ENDING_CHANGES) {
                    std::fs::remove_file(dir_entry.path()).unwrap();
                }
                #[cfg(unix)]
                if file_name.ends_with(ENDING_PROCESS_TRACE) {
                    std::fs::remove_file(dir_entry.path()).unwrap();
                }
            }
        }
    }

    fn selection_context<'context, 'arena: 'context>(
        &self,
        _ws: &Workspace<'_>,
        target_dir: &'context Path,
        arena: &'arena Arena<String>,
        units: &'context HashMap<Unit, Vec<UnitDep>>,
    ) -> Box<dyn SelectionContext<'context> + 'context> {
        Box::new(DynamicSelectionContext::new(target_dir, arena, units))
    }
}

pub fn exec(config: &mut Config, args: &ArgMatches) -> CliResult {
    super::exec(config, args, Selection::Precise(&DynamicMode))
}

pub(crate) struct DynamicSelectionContext<'arena, 'context> {
    selector: DynamicSelector<'arena, 'context>,
}

impl<'arena: 'context, 'context> DynamicSelectionContext<'arena, 'context> {
    fn new(
        target_dir: &'context Path,
        arena: &'arena Arena<String>,
        unit_graph: &'context HashMap<Unit, Vec<UnitDep>>,
    ) -> Self {
        Self {
            selector: DynamicSelector::new(target_dir, arena, unit_graph),
        }
    }
}

impl<'arena: 'context, 'context> SelectionContext<'context>
    for DynamicSelectionContext<'arena, 'context>
{
    fn selector(&mut self) -> &mut dyn Selector<'context> {
        &mut self.selector
    }
}

pub(crate) struct DynamicSelector<'arena, 'context> {
    cache: HashCache<'context, DependencyUnit<'context>, HashSet<ArenaIntern<'arena, String>>>,
    target_dir: &'context Path,
    arena: &'arena Arena<String>,
}

impl<'arena: 'context, 'context> DynamicSelector<'arena, 'context> {
    pub fn new(
        target_dir: &'context Path,
        arena: &'arena Arena<String>,
        unit_graph: &'context HashMap<Unit, Vec<UnitDep>>,
    ) -> Self {
        Self {
            cache: HashCache::recursive(
                move |cache: &mut HashCache<'context, _, _>, unit: &DependencyUnit<'context>| {
                    Self::import_changes(target_dir.to_path_buf(), arena, unit_graph, cache, unit)
                },
            ),
            target_dir,
            arena,
        }
    }

    fn import_changes(
        target_dir: PathBuf,
        arena: &'arena Arena<String>,
        unit_graph: &'context HashMap<Unit, Vec<UnitDep>>,
        cache: &mut HashCache<
            'context,
            DependencyUnit<'context>,
            HashSet<ArenaIntern<'arena, String>>,
        >,
        unit: &DependencyUnit<'context>,
    ) -> HashSet<ArenaIntern<'arena, String>> {
        let (unit, maybe_doctest_name) = match unit {
            DependencyUnit::Unit(u) => {
                debug_assert!(
                    matches!(u.mode, CompileMode::Test | CompileMode::Build),
                    "Got {:?} in {:?}",
                    u.mode,
                    u
                );
                (u, None)
            }
            DependencyUnit::DoctestUnit(u, s) => {
                debug_assert!(
                    matches!(u.mode, CompileMode::Doctest),
                    "Got {:?} in {:?}",
                    u.mode,
                    u
                );
                (u, Some(s.as_str()))
            }
        };

        let crate_name = unit.target.crate_name();
        let compile_mode = format!("{:?}", unit.mode);

        let changes_path = {
            let mut path = CacheKind::Dynamic.map(target_dir);
            CacheFileDescr::new(
                &crate_name,
                Some(&compile_mode),
                maybe_doctest_name,
                CacheFileKind::Changes,
            )
            .apply(&mut path);
            path
        };

        let mut changed_nodes: HashSet<ArenaIntern<'arena, String>> = read_to_string(changes_path)
            .ok()
            .map_or_else(HashSet::new, |s| {
                s.lines()
                    .map(ToString::to_string)
                    .map(|l| Arena::<String>::intern(arena, l))
                    .collect()
            });

        for other in unit_graph.get(unit).unwrap() {
            if other.unit.mode == CompileMode::Build // No point in including the graph of build scripts and proc_macros
                            && !other.unit.target.proc_macro()
            {
                let other_unit = DependencyUnit::Unit(&other.unit);
                let changes = cache.get(other_unit);

                changed_nodes.extend(changes);
            }
        }

        changed_nodes
    }
}

impl<'arena, 'context> DynamicSelector<'arena, 'context> {
    fn changed_nodes(
        &mut self,
        unit: DependencyUnit<'context>,
    ) -> &HashSet<ArenaIntern<'arena, String>> {
        self.cache.get(unit)
    }

    fn print_stats(
        &self,
        shell: &mut Shell,
        changed_nodes: &HashSet<ArenaIntern<'_, String>>,
        traced_tests: &HashSet<ArenaIntern<'_, String>>,
        tests_found: &HashSet<ArenaIntern<'_, String>>,
        affected_tests: &Vec<String>,
        start_time: Instant,
    ) -> CargoResult<()> {
        shell.status_header("Dynamic RTS")?;

        shell.concise(|shell| {
            shell.print_ansi_stderr(format!("{} changed;", changed_nodes.len()).as_bytes())
        })?;
        shell.concise(|shell| {
            shell.print_ansi_stderr(format!(" {} traces;", traced_tests.len()).as_bytes())
        })?;
        shell.concise(|shell| {
            shell.print_ansi_stderr(format!(" {} tests found;", tests_found.len()).as_bytes())
        })?;
        shell.concise(|shell| {
            shell.print_ansi_stderr(format!(" {} affected;", affected_tests.len()).as_bytes())
        })?;
        shell.concise(|shell| {
            shell.print_ansi_stderr(
                format!(" took {:.2}s\n", start_time.elapsed().as_secs_f64()).as_bytes(),
            )
        })?;

        shell.verbose(|shell| {
            shell.print_ansi_stderr(format!("took {:#?}\n", start_time.elapsed()).as_bytes())
        })?;
        shell.verbose(|shell| {
            shell.print_ansi_stderr(format!("\nChanged: {changed_nodes:?}\n").as_bytes())
        })?;
        shell.verbose(|shell| {
            shell.print_ansi_stderr(format!("Traces: {traced_tests:?}\n").as_bytes())
        })?;
        shell.verbose(|shell| {
            shell.print_ansi_stderr(format!("Tests found: {tests_found:?}\n").as_bytes())
        })?;
        shell.verbose(|shell| {
            shell.print_ansi_stderr(format!("Affected: {affected_tests:?}\n").as_bytes())
        })?;

        Ok(())
    }
}

impl<'arena, 'context> Selector<'context> for DynamicSelector<'arena, 'context> {
    fn select_tests(
        &mut self,
        test_unit: TestUnit<'context, 'context>,
        shell: &mut Shell,
        start_time: Instant,
    ) -> SelectionUnit {
        if self.check_retest_all() {
            return SelectionUnit::RetestAll;
        }

        let TestUnit(unit, test_info) = test_unit;
        let Some(test_info) = test_info else {
            panic!("Precise selction requires information about tests")
        };

        let mut changed_nodes = HashSet::new();
        let mut traced_tests = HashSet::new();
        let mut affected_tests = Vec::new();

        match test_info {
            TestInfo::Test(tests_found) => {
                debug_assert!(
                    matches!(unit.mode, CompileMode::Test),
                    "Got {:?} in {:?}",
                    unit.mode,
                    unit
                );

                let dependency_unit = DependencyUnit::Unit(unit);
                let changed = self.changed_nodes(dependency_unit).clone();

                let traces: HashMap<ArenaIntern<'_, String>, HashSet<ArenaIntern<'_, String>>> = {
                    let mut map = HashMap::new();

                    let path = CacheKind::Dynamic.map(self.target_dir.to_path_buf());

                    for test in &tests_found {
                        let descr =
                            CacheFileDescr::new(test.as_str(), None, None, CacheFileKind::Traces);
                        let mut path = path.clone();
                        descr.apply(&mut path);

                        if let Some(traces) = read_to_string(path.clone()).ok().map(|s| {
                            s.lines()
                                .map(ToString::to_string)
                                .map(|l| Arena::<String>::intern(self.arena, l))
                                .collect()
                        }) {
                            map.insert(*test, traces);
                        } else {
                            affected_tests.push(test.to_string());
                        }
                    }

                    map
                };

                changed_nodes.extend(changed.clone());

                affected_tests.extend(
                    traces
                        .iter()
                        .filter(|&(_test, traces)| {
                            traces.intersection(&changed_nodes).next().is_some()
                        })
                        .map(|(test, _traces)| test.to_string()),
                );

                traced_tests.extend(traces.into_keys());

                self.print_stats(
                    shell,
                    &changed_nodes,
                    &traced_tests,
                    &tests_found,
                    &affected_tests,
                    start_time,
                )
                .unwrap();
            }

            TestInfo::Doctest(tests) => {
                debug_assert!(
                    matches!(unit.mode, CompileMode::Doctest),
                    "Got {:?} in {:?}",
                    unit.mode,
                    unit
                );

                let mut tests_found = HashSet::new();
                let tests = tests.into_iter().map(|s| convert_doctest_name(&s)).unique();

                let (traces, no_traces) = {
                    let mut map_traces: HashMap<
                        ArenaIntern<'_, String>,
                        HashSet<ArenaIntern<'_, String>>,
                    > = HashMap::new();
                    let mut map_no_traces = HashMap::new();

                    for (trimmed, fn_name) in tests {
                        let dependency_unit = DependencyUnit::DoctestUnit(unit, fn_name.clone());
                        let changed = self.changed_nodes(dependency_unit).clone();

                        let path = CacheKind::Dynamic.map(self.target_dir.to_path_buf());

                        let interned = self.arena.intern(trimmed.clone());

                        tests_found.insert(interned);

                        let descr = CacheFileDescr::new(
                            fn_name.as_str(),
                            None,
                            None,
                            CacheFileKind::Traces,
                        );
                        let mut path = path.clone();
                        descr.apply(&mut path);

                        if let Some(traces) = read_to_string(path.clone()).ok().map(|s| {
                            s.lines()
                                .map(ToString::to_string)
                                .map(|l| Arena::<String>::intern(self.arena, l))
                                .collect()
                        }) {
                            map_traces.insert(interned, traces);
                        } else {
                            let name = DOCTEST_PREFIX.to_string() + &fn_name;
                            map_no_traces
                                .insert(interned, Arena::<String>::intern(self.arena, name));
                        }
                        changed_nodes.extend(changed.clone());
                    }

                    (map_traces, map_no_traces)
                };

                affected_tests.extend(
                    traces
                        .iter()
                        .filter(|&(_test, traces)| {
                            traces.intersection(&changed_nodes).next().is_some()
                        })
                        .map(|(test, _traces)| test.to_string()),
                );

                affected_tests.extend(
                    no_traces
                        .iter()
                        .filter(|(_test, name)| changed_nodes.contains(name))
                        .map(|(test, _name)| test.to_string()),
                );

                traced_tests.extend(traces.into_keys());

                self.print_stats(
                    shell,
                    &changed_nodes,
                    &traced_tests,
                    &tests_found,
                    &affected_tests,
                    start_time,
                )
                .unwrap();
            }
        };

        SelectionUnit::Precise(affected_tests)
    }

    fn cache_kind(&self) -> CacheKind {
        CacheKind::Dynamic
    }

    fn note(&self, shell: &mut Shell, test_args: &[&str]) {
        let mut args = vec!["--".to_string()];
        args.extend(test_args.iter().map(ToString::to_string));
        let opts: Option<TestOpts> = match parse_opts(&args) {
            Some(Ok(o)) => Some(o),
            _ => None,
        };

        let is_multithreaded = opts.and_then(|o| o.test_threads).map_or(true, |t| t > 1);

        let message = "Regression Test Selection using runtime traces\n";

        let info = if is_multithreaded {
            r#"IMPORTANT: Tests are run in parallel, isolating them in separate processes
This might not be feasible if tests rely on shared state.
You may use "--test-threads 1" as test option to run test sequentially instead."#
        } else {
            r"IMPORTANT: Tests are run sequentially (which does not require isolating them in separate processes)
This might lead to incomplete traces in the initialization of shared state."
        };

        shell.print_ansi_stderr(b"\n").unwrap();
        shell
            .status_with_color(
                "Dynamic RTS",
                message.to_string() + info,
                &cargo::util::style::NOTE,
            )
            .unwrap();
        shell.print_ansi_stderr(b"\n").unwrap();
    }

    fn doctest_callback_analysis(
        &self,
    ) -> fn(&mut cargo_util::ProcessBuilder, &std::path::Path, &cargo::core::compiler::Unit) {
        |p: &mut ProcessBuilder, target_dir: &Path, unit: &Unit| {
            let rustc_wrapper = {
                let mut path_buf =
                    std::env::current_exe().expect("current executable path invalid");
                path_buf.set_file_name("rustyrts-dynamic-doctest");
                path_buf
            };
            p.arg("-Z");
            p.arg("unstable-options");
            p.arg("--test-builder");
            p.arg(rustc_wrapper);

            p.env(ENV_TARGET_DIR, target_dir);
            p.env(ENV_DOCTESTED, unit.target.crate_name());
            p.env(ENV_COMPILE_MODE, format!("{:?}", unit.mode));
        }
    }

    fn doctest_callback_execution(
        &self,
    ) -> fn(&mut cargo_util::ProcessBuilder, &std::path::Path, &cargo::core::compiler::Unit) {
        |p: &mut ProcessBuilder, target_dir: &Path, unit: &Unit| {
            let rustc_wrapper = {
                let mut path_buf =
                    std::env::current_exe().expect("current executable path invalid");
                path_buf.set_file_name("rustyrts-dynamic-doctest");
                path_buf
            };
            p.arg("-Z");
            p.arg("unstable-options");
            p.arg("--test-builder");
            p.arg(rustc_wrapper);

            p.env(ENV_TARGET_DIR, target_dir);
            p.env(ENV_DOCTESTED, unit.target.crate_name());
            p.env(ENV_COMPILE_MODE, format!("{:?}", unit.mode));

            p.env(ENV_ONLY_INSTRUMENTATION, "true");
        }
    }
}
