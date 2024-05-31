use crate::{command_prelude::*, ops::TestExecutor};
use cargo::{
    core::{
        compiler::{
            unit_graph::{UnitDep, UnitGraph},
            Unit,
        },
        Shell,
    },
    util::Filesystem,
};
use cargo_util::ProcessBuilder;
use internment::{Arena, ArenaIntern};
use rustyrts::fs_utils::{CacheFileDescr, CacheFileKind, CacheKind};
use std::{
    collections::{HashMap, HashSet},
    fs::read_to_string,
    path::{Path, PathBuf},
    time::Instant,
    vec::Vec,
};

pub fn commands() -> Vec<Command> {
    vec![r#static::cli(), dynamic::cli(), clean::cli()]
}

pub type Exec = fn(&mut Config, &ArgMatches) -> CliResult;

pub fn command_exec(cmd: &str) -> Option<Exec> {
    let f = match cmd {
        "static" => r#static::exec,
        "dynamic" => dynamic::exec,
        "clean" => clean::exec,
        _ => return None,
    };
    Some(f)
}

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

    fn executor(&self, target_dir: PathBuf) -> TestExecutor;

    fn prepare_cache(&self, target_dir: &Path, unit_graph: &UnitGraph);

    fn clean_cache(&self, target_dir: &Path);

    fn selection_context<'context, 'arena: 'context>(
        &self,
        target_dir: &'context Path,
        arena: &'arena Arena<String>,
        units: &'context HashMap<Unit, Vec<UnitDep>>,
    ) -> Box<dyn SelectionContext<'context> + 'context>;
}

pub(crate) trait SelectionContext<'context> {
    fn selector(&mut self) -> &mut dyn Selector<'context>;
}

pub(crate) struct TestUnit<'unit, 'arena>(&'unit Unit, TestInfo<'arena>);

pub(crate) enum TestInfo<'arena> {
    Test(HashSet<ArenaIntern<'arena, String>>),
    Doctest(HashSet<String>),
}

impl<'unit, 'arena> TestUnit<'unit, 'arena> {
    pub fn test(unit: &'unit Unit, arena: &'arena Arena<String>, target_dir: &Path) -> Self {
        let path_buf = CacheKind::General.map(target_dir.to_path_buf());

        let crate_name = unit.target.crate_name();
        let compile_mode = format!("{:?}", unit.mode);

        let tests_path = {
            let mut path = path_buf.clone();
            CacheFileDescr::new(&crate_name, Some(&compile_mode), None, CacheFileKind::Tests)
                .apply(&mut path);
            path
        };

        let tests_found = read_to_string(&tests_path)
            .ok()
            .map(|s| {
                s.lines()
                    .filter(|l| !l.is_empty())
                    .map(|l| l.to_string())
                    .map(|l| Arena::<String>::intern(arena, l))
                    .collect()
            })
            .unwrap_or_else(|| {
                panic!(
                    "Did not find information on tests found for crate {:?}\nTried looking at {:?}",
                    crate_name, tests_path
                )
            });

        Self(unit, TestInfo::Test(tests_found))
    }

    pub fn doctest(unit: &'unit Unit, tests: HashSet<String>) -> Self {
        Self(unit, TestInfo::Doctest(tests))
    }
}

pub trait Selector<'context> {
    fn select_tests(
        &mut self,
        test: TestUnit<'context, 'context>,
        shell: &mut Shell,
        start_time: Instant,
    ) -> Vec<String>;

    fn cache_kind(&self) -> CacheKind;

    fn doctest_callback_analysis(&self) -> fn(&mut ProcessBuilder, &Path, &Unit);
    fn doctest_callback_execution(&self) -> fn(&mut ProcessBuilder, &Path, &Unit);
}

pub fn exec(config: &mut Config, args: &ArgMatches, mode: &dyn SelectionMode) -> CliResult {
    let ws = {
        let mut ws = args.workspace(config)?;

        if config.target_dir().unwrap().is_none() {
            let target_dir = mode.default_target_dir(ws.target_dir().into_path_unlocked());
            ws.set_target_dir(Filesystem::new(target_dir));
        }

        ws
    };

    let test_args = args.get_many::<String>("args").unwrap_or_default();
    let test_args = test_args.map(String::as_str).collect::<Vec<_>>();

    let mut compile_opts = args.compile_options(
        config,
        CompileMode::Test,
        Some(&ws),
        ProfileChecking::Custom,
    )?;

    compile_opts.build_config.requested_profile =
        args.get_profile_name(config, "test", ProfileChecking::Custom)?;

    crate::ops::run_tests(&ws, &compile_opts, &test_args, mode)
}

pub(crate) fn convert_doctest_name(test_name: &str) -> (String, String) {
    let (trimmed, _) = test_name.split_once(" - ").unwrap();
    let fn_name = trimmed
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect::<String>();
    (trimmed.to_string(), fn_name)
}
