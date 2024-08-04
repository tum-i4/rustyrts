use std::{
    boxed::Box,
    collections::{HashMap, HashSet},
    fmt::Display,
    fs::{read, read_dir, read_to_string, File},
    path::{Path, PathBuf},
    string::ToString,
    sync::Arc,
    time::Instant,
};

use cargo::{
    core::{
        compiler::{
            unit_graph::{UnitDep, UnitGraph},
            Executor, Unit,
        },
        Shell, Workspace,
    },
    util::command_prelude::*,
    CargoResult,
};

use cargo_util::ProcessBuilder;
use internment::{Arena, ArenaIntern};
use rustyrts::{
    callbacks_shared::DOCTEST_PREFIX,
    constants::{ENDING_CHANGES, ENV_COMPILE_MODE, ENV_DOCTESTED, ENV_TARGET, ENV_TARGET_DIR},
    fs_utils::{CacheFileDescr, CacheFileKind, CacheKind},
    static_rts::graph::{serialize::ArenaDeserializable, DependencyGraph},
};
use tracing::trace;

use crate::{commands::convert_doctest_name, ops::PreciseExecutor};

use super::{
    cache::HashCache, DependencyUnit, PreciseSelectionMode, SelectionContext, SelectionMode,
    SelectionUnit, Selector, TestInfo, TestUnit,
};

pub fn cli() -> Command {
    subcommand("static")
        .about(r"Perform regression test selection using a static technique, constructing a dependency graph

 ++ quite precise
 + does not tamper with binaries at all
 + no runtime overhead
 - cannot track dependencies of child processes
 / moderate compilation overhead

Consider using `cargo rustyrts dynamic` instead if your tests spawn additional processes!")
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

pub(crate) struct StaticMode;

impl SelectionMode for StaticMode {
    fn default_target_dir(&self, target_dir: PathBuf) -> std::path::PathBuf {
        let mut target_dir = target_dir;
        target_dir.push("static");
        target_dir
    }

    fn executor(&self, target_dir: PathBuf) -> Arc<dyn Executor> {
        let mut path_buf = std::env::current_exe().expect("current executable path invalid");
        path_buf.set_file_name("rustyrts-static");

        Arc::new(PreciseExecutor::new(path_buf, target_dir))
    }
}

impl PreciseSelectionMode for StaticMode {
    fn prepare_cache(&self, target_dir: &Path, unit_graph: &UnitGraph) {
        for kind in [CacheKind::General, CacheKind::Static] {
            let path = kind.map(target_dir.to_path_buf());
            std::fs::create_dir_all(path).expect("Failed to create cache directory");
        }

        for unit in unit_graph.keys() {
            match unit.mode {
                CompileMode::Test
                | CompileMode::Build
                | CompileMode::Doctest
                | CompileMode::RunCustomBuild => {}
                mode => panic!("Found unexpected compile mode, {:?}", mode),
            }
        }
    }

    fn clean_cache(&self, target_dir: &Path) {
        let path = CacheKind::Static.map(target_dir.to_path_buf());
        if let Ok(files) = read_dir(path) {
            for dir_entry in files.flatten() {
                let file_name = dir_entry.file_name();
                let file_name = file_name.to_str().unwrap();

                if file_name.ends_with(ENDING_CHANGES) {
                    std::fs::remove_file(dir_entry.path()).unwrap();
                }
            }
        }
    }

    fn selection_context<'context, 'arena: 'context>(
        &self,
        ws: &Workspace<'_>,
        target_dir: &'context Path,
        arena: &'arena Arena<String>,
        units: &'context HashMap<Unit, Vec<UnitDep>>,
    ) -> Box<dyn SelectionContext<'context> + 'context> {
        let verbose = ws.config().extra_verbose();
        Box::new(StaticSelectionContext::new(
            target_dir, arena, units, verbose,
        ))
    }
}

pub fn exec(config: &mut Config, args: &ArgMatches) -> CliResult {
    super::exec(config, args, super::Selection::Precise(&StaticMode))
}

pub(crate) struct StaticSelectionContext<'arena, 'context> {
    selector: StaticSelector<'arena, 'context>,
}

impl<'arena: 'context, 'context> StaticSelectionContext<'arena, 'context> {
    fn new(
        target_dir: &'context Path,
        arena: &'arena Arena<String>,
        unit_graph: &'context HashMap<Unit, Vec<UnitDep>>,
        pretty_print_graph: bool,
    ) -> Self {
        Self {
            selector: StaticSelector::new(target_dir, arena, unit_graph, pretty_print_graph),
        }
    }
}

impl<'arena, 'context> SelectionContext<'context> for StaticSelectionContext<'arena, 'context> {
    fn selector(&mut self) -> &mut dyn Selector<'context> {
        &mut self.selector
    }
}

pub(crate) struct StaticSelector<'arena, 'context> {
    cache: HashCache<'context, DependencyUnit<'context>, DependencyNode<'arena>>,
    arena: &'arena Arena<String>,
}

struct DependencyNode<'arena> {
    pub changes: HashSet<ArenaIntern<'arena, String>>,
    pub reached: HashSet<ArenaIntern<'arena, String>>,
    pub locally: HashSet<ArenaIntern<'arena, String>>,
}

impl<'arena: 'context, 'context> StaticSelector<'arena, 'context> {
    pub fn new(
        target_dir: &'context Path,
        arena: &'arena Arena<String>,
        unit_graph: &'context HashMap<Unit, Vec<UnitDep>>,
        pretty_print_graph: bool,
    ) -> Self {
        Self {
            cache: HashCache::recursive(
                move |cache: &mut HashCache<'context, _, _>, unit: &DependencyUnit<'context>| {
                    Self::import_graph(
                        target_dir.to_path_buf(),
                        arena,
                        unit_graph,
                        cache,
                        unit,
                        pretty_print_graph,
                    )
                },
            ),
            arena,
        }
    }

    fn import_graph(
        target_dir: PathBuf,
        arena: &'arena Arena<String>,
        unit_graph: &'context HashMap<Unit, Vec<UnitDep>>,
        cache: &mut HashCache<'context, DependencyUnit<'context>, DependencyNode<'arena>>,
        unit: &DependencyUnit<'context>,
        pretty_print_graph: bool,
    ) -> DependencyNode<'arena> {
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
        let target = unit.target.kind().description();

        let graph_path = {
            let mut path = CacheKind::Static.map(target_dir.clone());
            CacheFileDescr::new(
                &crate_name,
                Some(&compile_mode),
                Some(target),
                maybe_doctest_name,
                CacheFileKind::Graph,
            )
            .apply(&mut path);
            path
        };
        let changes_path = {
            let mut path = CacheKind::Static.map(target_dir.clone());
            CacheFileDescr::new(
                &crate_name,
                Some(&compile_mode),
                Some(target),
                maybe_doctest_name,
                CacheFileKind::Changes,
            )
            .apply(&mut path);
            path
        };

        let changed_nodes: HashSet<ArenaIntern<'arena, String>> = read_to_string(changes_path)
            .ok()
            .map_or_else(HashSet::new, |s| {
                s.lines()
                    .map(ToString::to_string)
                    .map(|l| Arena::<String>::intern(arena, l))
                    .collect()
            });

        let graph = read(graph_path.clone())
                        .ok()
                        .map_or_else(
                       || {
                            trace!(
                                "Did not find dependency graph for crate {:?} in mode {:?}\nTried reading from {:?}",
                                crate_name, compile_mode, graph_path
                            );
                            let mut graph = DependencyGraph::new(arena);
                            if let Some(doctest_name) = maybe_doctest_name {
                                let entry_name = DOCTEST_PREFIX.to_string() + doctest_name;
                                graph.add_edge(doctest_name.to_string(), entry_name, rustyrts::static_rts::graph::EdgeType::Trimmed);
                            }
                            graph
                        }, |s| DependencyGraph::deserialize(arena, &s).unwrap());

        if maybe_doctest_name.is_some() && graph_path.is_file() {
            std::fs::remove_file(graph_path).unwrap();
        }

        if pretty_print_graph {
            let pretty_path = {
                let mut path = CacheKind::Static.map(target_dir.clone());
                CacheFileDescr::new(
                    &crate_name,
                    Some(&compile_mode),
                    Some(target),
                    maybe_doctest_name,
                    CacheFileKind::PrettyGraph,
                )
                .apply(&mut path);
                path
            };

            let mut f =
                File::create(pretty_path).expect("Failed to create file for pretty-printing graph");
            graph.render_to(&mut f);
        }

        let mut starting_points = vec![changed_nodes.clone()];
        let mut reached = HashSet::new();
        let mut changed_nodes = changed_nodes;

        for other in unit_graph.get(unit).unwrap() {
            if other.unit.mode == CompileMode::Build {
                let other_unit = DependencyUnit::Unit(&other.unit);
                let DependencyNode {
                    changes,
                    reached: _,
                    locally,
                } = cache.get(other_unit);

                starting_points.push(locally.clone());
                changed_nodes.extend(changes);
            }
        }

        let locally = graph.reachable_nodes(starting_points.into_iter().flatten());
        reached.extend(&locally);

        DependencyNode {
            changes: changed_nodes,
            reached,
            locally,
        }
    }
}

impl<'arena, 'context> StaticSelector<'arena, 'context> {
    fn reachble_and_changed_nodes(
        &mut self,
        unit: DependencyUnit<'context>,
    ) -> (
        &HashSet<ArenaIntern<'arena, String>>,
        &HashSet<ArenaIntern<'arena, String>>,
        &HashSet<ArenaIntern<'arena, String>>,
    ) {
        let cached = self.cache.get(unit);
        (&cached.changes, &cached.locally, &cached.reached)
    }

    fn print_stats<T: Display>(
        &self,
        shell: &mut Shell,
        status: T,
        changed_nodes: &HashSet<ArenaIntern<'_, String>>,
        reachable_nodes: &HashSet<ArenaIntern<'_, String>>,
        tests_found: &HashSet<ArenaIntern<'_, String>>,
        affected_tests: &Vec<String>,
        start_time: Instant,
    ) -> CargoResult<()> {
        shell.status_header(status)?;

        shell.concise(|shell| {
            shell.print_ansi_stderr(format!("{} changed;", changed_nodes.len()).as_bytes())
        })?;
        shell.concise(|shell| {
            shell.print_ansi_stderr(format!(" {} reachable;", reachable_nodes.len()).as_bytes())
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

        let verbose_reachable = {
            if reachable_nodes.len() < 200 {
                format!("{reachable_nodes:?}")
            } else {
                format!("{}", reachable_nodes.len())
            }
        };

        shell.verbose(|shell| {
            shell.print_ansi_stderr(format!("took {:#?}\n", start_time.elapsed()).as_bytes())
        })?;
        shell.verbose(|shell| {
            shell.print_ansi_stderr(format!("\nChanged: {changed_nodes:?}\n").as_bytes())
        })?;
        shell.verbose(|shell| {
            shell.print_ansi_stderr(format!("Reachable: {verbose_reachable}\n").as_bytes())
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

impl<'arena, 'context> Selector<'context> for StaticSelector<'arena, 'context> {
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
            panic!("Precise selection requires information about tests")
        };

        let mut changed_nodes = HashSet::new();
        let mut reachable_nodes = HashSet::new();
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
                let (changed, locally, reachable) =
                    self.reachble_and_changed_nodes(dependency_unit);
                let affected = locally.intersection(&tests_found).map(ToString::to_string);

                changed_nodes.extend(changed.clone());
                reachable_nodes.extend(reachable.clone());
                affected_tests.extend(affected);

                self.print_stats(
                    shell,
                    "Static RTS",
                    &changed_nodes,
                    &reachable_nodes,
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

                for test in &tests {
                    let (trimmed, test_path) = convert_doctest_name(test);
                    let test = self.arena.intern(test_path.clone());

                    let dependency_unit = DependencyUnit::DoctestUnit(unit, test_path);
                    let (changed, locally, reachable) =
                        self.reachble_and_changed_nodes(dependency_unit);

                    changed_nodes.extend(changed.clone());
                    reachable_nodes.extend(reachable.clone());

                    if locally.contains(&test) {
                        affected_tests.push(trimmed);
                    }
                    tests_found.insert(test);
                }

                self.print_stats(
                    shell,
                    "Static RTS",
                    &changed_nodes,
                    &reachable_nodes,
                    &tests_found,
                    &affected_tests,
                    start_time,
                )
                .unwrap();
            }
        }

        SelectionUnit::Precise(affected_tests)
    }

    fn cache_kind(&self) -> CacheKind {
        CacheKind::Static
    }

    fn note(&self, shell: &mut Shell, _test_args: &[&str]) {
        let message = r"Regression Test Selection using a dependency graph";

        shell.print_ansi_stderr(b"\n").unwrap();
        shell
            .status_with_color("Static RTS", message, &cargo::util::style::NOTE)
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
                path_buf.set_file_name("rustyrts-static-doctest");
                path_buf
            };
            p.arg("-Z");
            p.arg("unstable-options");
            p.arg("--test-builder");
            p.arg(rustc_wrapper);

            p.env(ENV_TARGET_DIR, target_dir);
            p.env(ENV_DOCTESTED, unit.target.crate_name());
            p.env(ENV_COMPILE_MODE, format!("{:?}", unit.mode));
            p.env(ENV_TARGET, format!("{}", unit.target.kind().description()));
        }
    }

    fn doctest_callback_execution(
        &self,
    ) -> fn(&mut cargo_util::ProcessBuilder, &std::path::Path, &cargo::core::compiler::Unit) {
        |_p: &mut ProcessBuilder, _target_dirr: &Path, _unit: &Unit| {}
    }
}
