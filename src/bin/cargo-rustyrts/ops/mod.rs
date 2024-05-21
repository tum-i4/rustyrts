pub mod cargo_test;
pub use cargo_test::run_tests;
use rustyrts::constants::{ENV_DOCTESTED, ENV_TARGET_DIR};

use std::path::PathBuf;

use cargo::{
    core::compiler::{DefaultExecutor, Executor},
    CargoResult,
};

struct TestExecutor {
    target_dir: PathBuf,
    delegate: DefaultExecutor,
}

impl TestExecutor {
    fn new(target_dir: PathBuf) -> Self {
        Self {
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
        #[allow(mutable_transmutes)]
        let cmd: &mut cargo_util::ProcessBuilder = unsafe { std::mem::transmute(cmd) };
        cmd.env(ENV_TARGET_DIR, &self.target_dir);
        if target.doctested() {
            cmd.env(ENV_DOCTESTED, "true");
        }

        self.delegate
            .exec(cmd, id, target, mode, on_stdout_line, on_stderr_line)
    }
}
