pub use cargo_test::run_tests;
pub mod cargo_test;

use rustyrts::constants::{
    ENV_COMPILE_MODE, ENV_DOCTESTED, ENV_SKIP_ANALYSIS, ENV_SKIP_INSTRUMENTATION, ENV_TARGET_DIR,
};
use tracing::debug;

use std::path::PathBuf;

use cargo::{
    core::compiler::{DefaultExecutor, Executor},
    CargoResult,
};

pub(crate) struct TestExecutor {
    cmd: PathBuf,
    target_dir: PathBuf,
    delegate: DefaultExecutor,
}

impl TestExecutor {
    pub fn new(cmd: PathBuf, target_dir: PathBuf) -> Self {
        Self {
            cmd,
            target_dir,
            delegate: DefaultExecutor,
        }
    }
}

impl Executor for TestExecutor {
    fn exec(
        &self,
        cmd: &cargo_util::ProcessBuilder,
        id: cargo::core::PackageId,
        target: &cargo::core::Target,
        mode: cargo::util::command_prelude::CompileMode,
        on_stdout_line: &mut dyn FnMut(&str) -> CargoResult<()>,
        on_stderr_line: &mut dyn FnMut(&str) -> CargoResult<()>,
    ) -> CargoResult<()> {
        debug!("Got target {:?}, mode {:?}", target, mode);

        if mode.is_run_custom_build() {
            self.delegate
                .exec(cmd, id, target, mode, on_stdout_line, on_stderr_line)
        } else {
            let mut cmd = cmd.clone();
            cmd.program(&self.cmd);

            cmd.env(ENV_TARGET_DIR, &self.target_dir);
            cmd.env(ENV_COMPILE_MODE, format!("{mode:?}"));
            if target.doctested() {
                cmd.env(ENV_DOCTESTED, "true");
            }

            if target.is_custom_build() {
                cmd.env(ENV_SKIP_ANALYSIS, "true");
                cmd.env(ENV_SKIP_INSTRUMENTATION, "true");
            }

            self.delegate
                .exec(&cmd, id, target, mode, on_stdout_line, on_stderr_line)
        }
    }
}
