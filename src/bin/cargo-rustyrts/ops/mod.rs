pub use cargo_test::run_tests;
pub mod cargo_test;

use rustyrts::constants::{
    ENV_COMPILE_MODE, ENV_DOCTESTED, ENV_SKIP_ANALYSIS, ENV_SKIP_INSTRUMENTATION, ENV_TARGET_DIR,
};
use tracing::debug;

use std::{collections::HashSet, path::PathBuf, sync::Mutex};

use cargo::{
    core::{
        compiler::{DefaultExecutor, Executor},
        Target,
    },
    CargoResult,
};

pub(crate) struct PreciseExecutor {
    cmd: PathBuf,
    target_dir: PathBuf,
    delegate: DefaultExecutor,
}

impl PreciseExecutor {
    pub fn new(cmd: PathBuf, target_dir: PathBuf) -> Self {
        Self {
            cmd,
            target_dir,
            delegate: DefaultExecutor,
        }
    }
}

impl Executor for PreciseExecutor {
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

pub(crate) struct CrateLevelExecutor {
    compiled_targets: Mutex<HashSet<Target>>,
    delegate: DefaultExecutor,
}

impl CrateLevelExecutor {
    pub(crate) fn new() -> Self {
        Self {
            compiled_targets: Mutex::new(HashSet::new()),
            delegate: DefaultExecutor,
        }
    }

    pub(crate) fn compiled_targets(self) -> HashSet<Target> {
        self.compiled_targets.into_inner().unwrap()
    }
}

impl Executor for CrateLevelExecutor {
    fn exec(
        &self,
        cmd: &cargo_util::ProcessBuilder,
        id: cargo::core::PackageId,
        target: &cargo::core::Target,
        mode: cargo::util::command_prelude::CompileMode,
        on_stdout_line: &mut dyn FnMut(&str) -> CargoResult<()>,
        on_stderr_line: &mut dyn FnMut(&str) -> CargoResult<()>,
    ) -> CargoResult<()> {
        if mode == cargo::util::command_prelude::CompileMode::Test {
            self.compiled_targets.lock().unwrap().insert(target.clone());
        }

        self.delegate
            .exec(&cmd, id, target, mode, on_stdout_line, on_stderr_line)
    }
}
