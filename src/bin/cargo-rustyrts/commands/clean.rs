use std::ffi::OsString;

use cargo::util::command_prelude::*;

use super::{dynamic::DynamicMode, r#static::StaticMode, SelectionMode};

pub fn cli() -> Command {
    subcommand("clean").about("Clean cache directory")
}

fn cargo() -> std::process::Command {
    std::process::Command::new(std::env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo")))
}

pub fn exec(config: &mut Config, args: &ArgMatches) -> CliResult {
    let ws = args.workspace(config)?;

    for mode in [
        &StaticMode as &dyn SelectionMode,
        &DynamicMode as &dyn SelectionMode,
    ] {
        let target_dir = mode.default_target_dir(ws.target_dir().into_path_unlocked());

        let mut cmd = cargo();
        cmd.arg("clean");
        cmd.env("CARGO_TARGET_DIR", target_dir);
        cmd.status().unwrap();
    }

    Ok(())
}
