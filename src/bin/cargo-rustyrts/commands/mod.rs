extern crate cargo;

use crate::{command_prelude::*, doctest_rts::run_analysis_doctests};
use cargo::{
    core::{
        compiler::{
            unit_graph::{UnitDep, UnitGraph},
            Compilation, Doctest, Executor, Unit,
        },
        Shell, Workspace,
    },
    util::Filesystem,
};
use cargo_util::ProcessBuilder;
use internment::{Arena, ArenaIntern};
use rustyrts::{
    constants::ENV_RETEST_ALL,
    fs_utils::{CacheFileDescr, CacheFileKind, CacheKind},
};
use std::{
    collections::{HashMap, HashSet},
    fs::read_to_string,
    path::{Path, PathBuf},
    sync::Arc,
    time::Instant,
    vec::Vec,
};

pub fn commands() -> Vec<Command> {
    vec![basic::cli(), r#static::cli(), dynamic::cli(), clean::cli()]
}

pub type Exec = fn(&mut Config, &ArgMatches) -> CliResult;

pub fn command_exec(cmd: &str) -> Option<Exec> {
    let f = match cmd {
        "basic" => r#basic::exec,
        "static" => r#static::exec,
        "dynamic" => dynamic::exec,
        "clean" => clean::exec,
        _ => return None,
    };
    Some(f)
}

pub(crate) mod basic;
pub(crate) mod clean;
pub(crate) mod dynamic;
pub(crate) mod r#static;

mod cache;

#[derive(PartialEq, Eq, Hash, Clone)]
enum DependencyUnit<'context> {
    Unit(&'context Unit),
    DoctestUnit(&'context Unit, String),
}

pub(crate) trait SelectionMode {
    fn default_target_dir(&self, target_dir: PathBuf) -> PathBuf;

    fn executor(&self, target_dir: PathBuf) -> Arc<dyn Executor>;
}

pub enum Selection<'mode> {
    Precise(&'mode dyn PreciseSelectionMode),
    CrateLevel(&'mode dyn CrateLevelSelectionMode),
}

impl<'mode> Selection<'mode> {
    pub(crate) fn default_target_dir(&self, orig: PathBuf) -> PathBuf {
        match self {
            Selection::Precise(mode) => mode.default_target_dir(orig),
            Selection::CrateLevel(mode) => mode.default_target_dir(orig),
        }
    }

    pub(crate) fn executor(&self, target_dir: &PathBuf) -> Arc<dyn Executor> {
        match self {
            Selection::Precise(mode) => mode.executor(target_dir.clone()),
            Selection::CrateLevel(mode) => mode.executor(target_dir.clone()),
        }
    }

    pub(crate) fn selection_context<'target_dir: 'mode, 'arena: 'mode, 'unit_graph: 'mode>(
        &self,
        ws: &Workspace,
        target_dir: &'target_dir Path,
        arena: &'arena Arena<String>,
        unit_graph: &'unit_graph HashMap<Unit, Vec<UnitDep>>,
    ) -> Box<dyn SelectionContext<'mode> + 'mode> {
        match self {
            Selection::Precise(mode) => mode.selection_context(ws, &target_dir, &arena, unit_graph),
            Selection::CrateLevel(mode) => mode.selection_context(),
        }
    }
}

pub(crate) trait PreciseSelectionMode: SelectionMode {
    fn selection_context<'context, 'arena: 'context>(
        &self,
        ws: &Workspace<'_>,
        target_dir: &'context Path,
        arena: &'arena Arena<String>,
        units: &'context HashMap<Unit, Vec<UnitDep>>,
    ) -> Box<dyn SelectionContext<'context> + 'context>;

    fn prepare_cache(&self, target_dir: &Path, unit_graph: &UnitGraph);

    fn clean_cache(&self, target_dir: &Path);
}

pub(crate) trait CrateLevelSelectionMode: SelectionMode {
    fn selection_context<'context>(
        &'context self,
    ) -> Box<dyn SelectionContext<'context> + 'context>;
}

pub trait SelectionContext<'context> {
    fn selector(&mut self) -> &mut dyn Selector<'context>;
}

pub struct TestUnit<'unit, 'arena>(pub &'unit Unit, pub Option<TestInfo<'arena>>);

pub enum TestInfo<'arena> {
    Test(HashSet<ArenaIntern<'arena, String>>),
    Doctest(HashSet<String>),
}

pub enum SelectionUnit {
    RetestAll,
    CrateLevel { execute_tests: bool },
    Precise(Vec<String>),
}

pub trait Selector<'context> {
    fn test_info<'arena>(
        &self,
        unit: &Unit,
        arena: &'arena Arena<String>,
        target_dir: &Path,
    ) -> Option<TestInfo<'arena>> {
        let path_buf = CacheKind::General.map(target_dir.to_path_buf());

        let crate_name = unit.target.crate_name();
        let compile_mode = format!("{:?}", unit.mode);
        let target = unit.target.kind().description();

        let tests_path = {
            let mut path = path_buf;
            CacheFileDescr::new(
                &crate_name,
                Some(&compile_mode),
                Some(&target),
                None,
                CacheFileKind::Tests,
            )
            .apply(&mut path);
            path
        };

        let tests_found = read_to_string(&tests_path)
            .ok()
            .map_or_else(|| panic!(
                    "Did not find information on tests found for crate {crate_name:?}\nTried looking at {tests_path:?}"
                ),|s| {
                s.lines()
                    .filter(|l| !l.is_empty())
                    .map(std::string::ToString::to_string)
                    .map(|l| Arena::<String>::intern(arena, l))
                    .collect()
            });

        Some(TestInfo::Test(tests_found))
    }

    fn doctest_info<'arena>(
        &self,
        ws: &Workspace,
        test_args: &[&str],
        compilation: &Compilation,
        target_dir: &Path,
        doctest_info: &Doctest,
    ) -> Result<Option<TestInfo<'arena>>, CliError> {
        let tests = run_analysis_doctests(
            ws,
            test_args,
            compilation,
            target_dir,
            doctest_info,
            self.cache_kind(),
            self.doctest_callback_analysis(),
        )?;
        Ok(Some(TestInfo::Doctest(tests)))
    }

    fn select_tests(
        &mut self,
        test: TestUnit<'context, 'context>,
        shell: &mut Shell,
        start_time: Instant,
    ) -> SelectionUnit;

    fn cache_kind(&self) -> CacheKind;

    fn note(&self, shell: &mut Shell, test_args: &[&str]);
    fn doctest_callback_analysis(&self) -> fn(&mut ProcessBuilder, &Path, &Unit);
    fn doctest_callback_execution(&self) -> fn(&mut ProcessBuilder, &Path, &Unit);

    fn check_retest_all(&self) -> bool {
        std::env::var(ENV_RETEST_ALL).is_ok()
    }
}

pub fn exec(config: &Config, args: &ArgMatches, selection: Selection) -> CliResult {
    let ws = {
        let mut ws = args.workspace(config)?;

        if config.target_dir().unwrap().is_none() {
            let target_dir = selection.default_target_dir(ws.target_dir().into_path_unlocked());
            ws.set_target_dir(Filesystem::new(target_dir));
        }

        ws
    };

    let test_args = args.get_many::<String>("args").unwrap_or_default();
    let test_args = test_args.map(String::as_str).collect::<Vec<_>>();

    if test_args.iter().any(|s| s == &"--exact") {
        return Err(anyhow::format_err!(
            "Regression Test Selection is incompatible to using --exact"
        )
        .into());
    }

    let mut compile_opts = args.compile_options(
        config,
        CompileMode::Test,
        Some(&ws),
        ProfileChecking::Custom,
    )?;

    compile_opts.build_config.requested_profile =
        args.get_profile_name(config, "test", ProfileChecking::Custom)?;

    let no_run = args.flag("no-run");
    let doc = args.flag("doc");

    if doc {
        if compile_opts.filter.is_specific() {
            return Err(
                anyhow::format_err!("Can't mix --doc with other target selecting options").into(),
            );
        }
        if no_run {
            return Err(anyhow::format_err!("Can't skip running doc tests with --no-run").into());
        }
        compile_opts.build_config.mode = CompileMode::Doctest;
        compile_opts.filter = cargo::ops::CompileFilter::lib_only();
    }

    if compile_opts.filter.is_specific() {
        config.shell().warn(r"You are trying to manipulate the set of executed tests via target selection (--lib, --bins, --examples, ...).
This is dangerous since changes may also affect tests outside of the crates that are compiled in this compiler session. Any affected tests not contained in your target selection, which are affected by changes in a crate that is compiled in this compiler session will definitely not be executed now and may also be missed in a subsequent invocation!
Proceed with caution!!!
").unwrap();
    }

    let ops = cargo::ops::TestOptions {
        no_run,
        no_fail_fast: true,
        compile_opts,
    };

    crate::ops::run_tests(&ws, &ops, &test_args, selection)
}

pub fn convert_doctest_name(test_name: &str) -> (String, String) {
    let (trimmed, _) = test_name.split_once(" - ").unwrap();
    let fn_name = trimmed
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect::<String>();
    (trimmed.to_string(), fn_name)
}
