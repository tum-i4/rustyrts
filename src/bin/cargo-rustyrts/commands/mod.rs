use std::{path::PathBuf, vec::Vec};

use cargo::{
    core::Workspace,
    ops::{self, CompileFilter, TestOptions},
    util::{
        config::{CargoBuildConfig, ConfigRelativePath, Definition, Value},
        Filesystem,
    },
};

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

pub mod clean;
pub mod dynamic;
pub mod r#static;

pub trait SelectionMode {
    fn cmd(&self) -> PathBuf;

    fn default_target_dir(&self, target_dir: PathBuf) -> PathBuf;

    fn select_tests(&self, config: &Config, target_dir: PathBuf) -> Vec<String>;

    fn select_doctests(&self, config: &Config, target_dir: PathBuf) -> Vec<String>;

    fn clean_intermediate_files(&self, target_dir: PathBuf);
}

pub fn exec(config: &mut Config, args: &ArgMatches, mode: &dyn SelectionMode) -> CliResult {
    let config_test = make_config(
        config,
        args,
        |buf| mode.default_target_dir(buf),
        mode.cmd(),
        CompileFilter::all_test_targets(),
    )?;

    let config_doctest = make_config(
        config,
        args,
        |mut buf| {
            buf.push("doctest");
            buf
        },
        mode.cmd(), // TODO
        CompileFilter::lib_only(),
    )?;

    crate::ops::run_tests(config_test, None, &[], mode)
}

fn make_config<'a>(
    config: &'a Config,
    args: &'a ArgMatches,
    target_dir_callback: impl FnOnce(PathBuf) -> PathBuf,
    rustc_wrapper: PathBuf,
    compile_filter: CompileFilter,
) -> Result<(Workspace<'a>, TestOptions, PathBuf), CliError> {
    let ws = {
        let mut ws = args.workspace(config)?;

        if config.target_dir().unwrap().is_none() {
            let target_dir = target_dir_callback(ws.target_dir().into_path_unlocked());
            ws.set_target_dir(Filesystem::new(target_dir));
        }

        #[allow(mutable_transmutes)]
        let build_config: &mut CargoBuildConfig =
            unsafe { std::mem::transmute(ws.config().build_config().unwrap()) };
        build_config.rustc_wrapper = Some(ConfigRelativePath::new(Value {
            val: rustc_wrapper
                .to_str()
                .ok_or_else(|| CliError::code(-1))?
                .to_string(),
            definition: Definition::Cli(None),
        }));

        ws
    };
    let target_dir = ws.target_dir().into_path_unlocked();

    let ops = {
        let mut compile_opts = args.compile_options(
            config,
            CompileMode::Test,
            Some(&ws),
            ProfileChecking::Custom,
        )?;
        compile_opts.build_config.requested_profile =
            args.get_profile_name(config, "test", ProfileChecking::Custom)?;
        compile_opts.filter = compile_filter;

        ops::TestOptions {
            no_run: false,
            no_fail_fast: true,
            compile_opts,
        }
    };

    Ok((ws, ops, target_dir))
}
