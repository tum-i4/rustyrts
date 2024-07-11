use core::panic;
use std::{
    boxed::Box,
    collections::HashSet,
    fmt::Display,
    path::{Path, PathBuf},
    sync::Arc,
    time::Instant,
};

use cargo::{
    core::{
        compiler::{Compilation, Doctest, Executor, Unit},
        Shell, Target, Workspace,
    },
    util::command_prelude::*,
    CargoResult,
};

use cargo_util::ProcessBuilder;
use internment::Arena;

use rustyrts::fs_utils::CacheKind;

use crate::ops::CrateLevelExecutor;

use self::lazy_transform::LazyTransform;

use super::{
    CrateLevelSelectionMode, SelectionContext, SelectionMode, SelectionUnit, Selector, TestInfo,
    TestUnit,
};

pub fn cli() -> Command {
    subcommand("basic")
        .about(
            r"Perform regression test selection at crate level 

 + moderatley precise
 + no compilation overhead
 + no runtime overhead
 + does not tamper with binaries at all
 - cannot track dependencies of child processes

Consider using `cargo rustyrts static` instead for increased precision!
Consider using `cargo rustyrts dynamic` instead if your tests spawn additional processes!",
        )
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

mod lazy_transform {
    use std::cell::UnsafeCell;

    enum Value<T, U> {
        Uninit,
        Init(T),
        Final(U),
    }

    impl<T, U> Value<T, U> {
        fn take(&mut self) -> Self {
            std::mem::replace(self, Value::Uninit)
        }
    }

    pub struct LazyTransform<T, U> {
        value: UnsafeCell<Value<T, U>>,
    }

    impl<T, U> LazyTransform<T, U> {
        pub fn new<'a>(init: T) -> Self {
            LazyTransform {
                value: Value::Init(init).into(),
            }
        }

        pub fn get(&self) -> &T {
            let value = unsafe { &*self.value.get() };
            match value {
                Value::Uninit => unreachable!(),
                Value::Init(v) => v,
                Value::Final(_) => panic!("Value has been transformed"),
            }
        }

        pub fn transform<F: Fn(T) -> U>(&self, transform: F) -> &U {
            let value = unsafe { &mut *self.value.get() };
            match value.take() {
                Value::Uninit => unreachable!(),
                Value::Init(v) => {
                    let final_value = (transform)(v);
                    *value = Value::Final(final_value).into();
                }
                Value::Final(v) => *value = Value::Final(v),
            }
            match value {
                Value::Uninit => unreachable!(),
                Value::Init(_) => unreachable!(),
                Value::Final(ref final_value) => final_value,
            }
        }
    }
}

pub(crate) struct BasicMode {
    executor: LazyTransform<Arc<CrateLevelExecutor>, HashSet<Target>>,
}

impl BasicMode {
    pub(crate) fn new() -> Self {
        Self {
            executor: LazyTransform::new(Arc::new(CrateLevelExecutor::new())),
        }
    }
}

impl SelectionMode for BasicMode {
    fn default_target_dir(&self, target_dir: PathBuf) -> std::path::PathBuf {
        let mut target_dir = target_dir;
        target_dir.push("basic");
        target_dir
    }

    fn executor(&self, _target_dir: PathBuf) -> Arc<dyn Executor> {
        self.executor.get().clone()
    }
}

impl CrateLevelSelectionMode for BasicMode {
    fn selection_context<'context>(
        &'context self,
    ) -> Box<dyn SelectionContext<'context> + 'context> {
        Box::new(BasicSelectionContext::new(self.executor.transform(
            |exec| {
                let Ok(exec) = Arc::try_unwrap(exec) else {
                    panic!("Failed to unwrap arc")
                };
                exec.compiled_targets()
            },
        )))
    }
}

pub fn exec(config: &mut Config, args: &ArgMatches) -> CliResult {
    super::exec(
        config,
        args,
        super::Selection::CrateLevel(&BasicMode::new()),
    )
}

pub(crate) struct BasicSelectionContext<'context> {
    selector: BasicSelector<'context>,
}

impl<'context> BasicSelectionContext<'context> {
    fn new(compiled_targets: &'context HashSet<Target>) -> Self {
        Self {
            selector: BasicSelector::new(compiled_targets),
        }
    }
}

impl<'context> SelectionContext<'context> for BasicSelectionContext<'context> {
    fn selector(&mut self) -> &mut dyn Selector<'context> {
        &mut self.selector
    }
}

pub(crate) struct BasicSelector<'context> {
    compiled_targets: &'context HashSet<Target>,
}

impl<'context> BasicSelector<'context> {
    pub fn new(compiled_targets: &'context HashSet<Target>) -> Self {
        Self { compiled_targets }
    }
}

impl<'context> BasicSelector<'context> {
    fn print_stats<T: Display>(
        &self,
        shell: &mut Shell,
        status: T,
        has_been_compiled: bool,
        start_time: Instant,
    ) -> CargoResult<()> {
        shell.status_header(status)?;

        shell.verbose(|shell| {
            shell.print_ansi_stderr(format!("took {:#?}\n", start_time.elapsed()).as_bytes())
        })?;
        if has_been_compiled {
            shell.concise(|shell| {
                shell.print_ansi_stderr(
                    format!("Crate has been (re-)compiled, executing all tests;").as_bytes(),
                )
            })?;
            shell.verbose(|shell| {
                shell.print_ansi_stderr(
                    format!("Crate has been (re-)compiled, executing all tests\n").as_bytes(),
                )
            })?;
        } else {
            shell.concise(|shell| {
                shell.print_ansi_stderr(
                    format!("Crate has not been (re-)compiled, not executing tests;").as_bytes(),
                )
            })?;
            shell.verbose(|shell| {
                shell.print_ansi_stderr(
                    format!("Crate has not been (re-)compiled, not executing tests\n").as_bytes(),
                )
            })?;
        }
        shell.concise(|shell| {
            shell.print_ansi_stderr(
                format!(" took {:.2}s\n", start_time.elapsed().as_secs_f64()).as_bytes(),
            )
        })?;

        Ok(())
    }
}

impl<'context> Selector<'context> for BasicSelector<'context> {
    fn test_info<'arena>(
        &self,
        _unit: &Unit,
        _arena: &'arena Arena<String>,
        _target_dir: &Path,
    ) -> Option<TestInfo<'arena>> {
        None
    }

    fn doctest_info<'arena>(
        &self,
        _ws: &Workspace,
        _test_args: &[&str],
        _compilation: &Compilation,
        _target_dir: &Path,
        _doctest_info: &Doctest,
    ) -> Result<Option<TestInfo<'arena>>, CliError> {
        Ok(None)
    }

    fn select_tests(
        &mut self,
        test_unit: TestUnit<'context, 'context>,
        shell: &mut Shell,
        start_time: Instant,
    ) -> SelectionUnit {
        let TestUnit(unit, test_info) = test_unit;
        debug_assert!(test_info.is_none());

        let execute_tests = self.compiled_targets.contains(&unit.target);
        self.print_stats(shell, "Basic RTS", execute_tests, start_time)
            .unwrap();
        SelectionUnit::CrateLevel { execute_tests }
    }

    fn cache_kind(&self) -> CacheKind {
        unreachable!()
    }

    fn note(&self, shell: &mut Shell, _test_args: &[&str]) {
        let message = r"Regression Test Selection at crate level";

        shell.print_ansi_stderr(b"\n").unwrap();
        shell
            .status_with_color("Basic RTS", message, &cargo::util::style::NOTE)
            .unwrap();
        shell.print_ansi_stderr(b"\n").unwrap();
    }

    fn doctest_callback_analysis(
        &self,
    ) -> fn(&mut cargo_util::ProcessBuilder, &std::path::Path, &cargo::core::compiler::Unit) {
        return |_p, _target_dir, _unit| {};
    }

    fn doctest_callback_execution(
        &self,
    ) -> fn(&mut cargo_util::ProcessBuilder, &std::path::Path, &cargo::core::compiler::Unit) {
        |_p: &mut ProcessBuilder, _target_dirr: &Path, _unit: &Unit| {}
    }
}
