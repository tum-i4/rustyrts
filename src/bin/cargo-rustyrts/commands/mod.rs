use std::{path::PathBuf, vec::Vec};

use cargo::{
    core::{compiler::Compilation, Workspace},
    util::Filesystem,
};
use itertools::Itertools;
use rustyrts::doctest_rts::run_analysis_doctests;

use crate::command_prelude::*;

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

pub trait SelectionMode {
    fn cmd(&self) -> PathBuf;

    fn default_target_dir(&self, target_dir: PathBuf) -> PathBuf;

    fn select_tests(&self, config: &Config, target_dir: PathBuf) -> Vec<String>;

    fn clean_intermediate_files(&self, target_dir: PathBuf);

    fn select_doc_tests(
        &self,
        ws: &Workspace<'_>,
        compilation: &Compilation,
        target_dir: PathBuf,
    ) -> Vec<String> {
        let changed_doctests = run_analysis_doctests(ws, compilation, target_dir);

        changed_doctests.into_iter().collect_vec()
    }
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
    let target_dir = ws.target_dir().into_path_unlocked();

    let mut compile_opts = args.compile_options(
        config,
        CompileMode::Test,
        Some(&ws),
        ProfileChecking::Custom,
    )?;

    compile_opts.build_config.requested_profile =
        args.get_profile_name(config, "test", ProfileChecking::Custom)?;

    crate::ops::run_tests(&ws, &compile_opts, target_dir, &[], mode)
}
