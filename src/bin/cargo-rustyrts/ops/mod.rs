pub mod cargo_test;

use std::{path::PathBuf, sync::Arc};

use cargo::{
    core::{
        compiler::{Compilation, DefaultExecutor, Executor},
        Workspace,
    },
    ops::{compile_with_exec, CompileOptions},
    CargoResult,
};

pub use self::cargo_test::run_tests;

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
        println!("Injecting target dir");

        #[allow(mutable_transmutes)]
        let cmd: &mut cargo_util::ProcessBuilder = unsafe { std::mem::transmute(cmd) };
        cmd.env("CARGO_TARGET_DIR", &self.target_dir);

        self.delegate
            .exec(cmd, id, target, mode, on_stdout_line, on_stderr_line)
    }
}

struct DoctestExecutor {
    target_dir: PathBuf,
    delegate: DefaultExecutor,
}

impl DoctestExecutor {
    fn new(target_dir: PathBuf) -> Self {
        Self {
            target_dir,
            delegate: DefaultExecutor,
        }
    }
}

impl Executor for DoctestExecutor {
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
        cmd.env("CARGO_TARGET_DIR", &self.target_dir);

        self.delegate
            .exec(cmd, id, target, mode, on_stdout_line, on_stderr_line)
    }
}
