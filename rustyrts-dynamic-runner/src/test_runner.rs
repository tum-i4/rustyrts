use libc::close;
use std::io::{self, stdout};
use std::os::fd::AsRawFd;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::process::{self};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use test::test::{parse_opts, TestExecTime, TestTimeOptions};
use test::{OutputFormat, ShouldPanic, TestDesc, TestDescAndFn, TestOpts};

use crate::constants::DESC_FLAG;
use crate::libtest::{
    calc_result, len_if_padded, make_owned_test, ConsoleTestState, CustomTestDesc, JsonFormatter,
    JunitFormatter, OutputFormatter, OutputLocation, PrettyFormatter, TerseFormatter, TestResult,
    TestSuiteExecTime, ERROR_EXIT_CODE,
};

#[cfg(unix)]
use crate::pipe::create_pipes;

#[cfg(unix)]
use crate::util::waitpid_wrapper;

#[cfg(unix)]
const UNUSUAL_EXIT_CODE: libc::c_int = 15;

#[no_mangle]
pub fn rustyrts_runner(tests: &[&test::TestDescAndFn]) {
    let args: Vec<String> = std::env::args().collect::<Vec<_>>();

    if args
        .iter()
        .skip_while(|arg| *arg != "--")
        .any(|arg| arg == DESC_FLAG)
    {
        let test_descriptions: Vec<CustomTestDesc> = tests
            .iter()
            .map(|t| t.desc.clone())
            .map(|desc| unsafe { std::mem::transmute(desc) })
            .collect();
        println!(
            "Test descriptions: {}",
            serde_json::to_string(&test_descriptions).unwrap()
        );
    }

    let opts: TestOpts = parse_opts(&args).unwrap().unwrap();

    // Exclude some options that just do not fit into RTS
    assert!(
        opts.run_tests,
        "WARNING: Running RustyRTS without executing any tests may result in unsafe behavior (i.e. some failing tests may be overseen)."
    );
    assert!(
        opts.filter_exact,
        "ERROR: RustyRTS is supposed to select tests using --exact."
    );
    assert!(!opts.fail_fast, "WARNING: Running RustyRTS without --no-fail-fast may result in unsafe behavior (i.e. some failing tests may be overseen).");
    assert!(
        !opts.bench_benchmarks,
        "ERROR: RustyRTS is not supposed to run benchmarks."
    );
    assert!(opts.skip.is_empty(), "WARNING: RustyRTS does not support excluding tests from cli. Tests may be ignored using #[ignore].");
    assert!(
        !opts.shuffle,
        "WARNING: RustyRTS does not support shuffling tests."
    );

    let affected_tests: &Vec<String> = &opts.filters;

    let all_tests: Vec<_> = tests.iter().map(make_owned_test).collect();
    let mut tests: Vec<TestDescAndFn> = Vec::new();

    let mut state = ConsoleTestState::new(&opts).unwrap();

    let is_instant_supported = !cfg!(target_family = "wasm") && !cfg!(miri);
    let start_time = is_instant_supported.then(Instant::now);

    for test in all_tests {
        if opts.exclude_should_panic && test.desc.should_panic != ShouldPanic::No {
            // 1. filter tests that should panic if specified so
            state.filtered_out += 1;
        } else {
            // 2. Check if test is affected
            if affected_tests.contains(&test.desc.name.as_slice().to_string()) {
                // 3. Check if test is not ignored (or specified to run ignored test)
                match opts.run_ignored {
                    test::RunIgnored::Yes => tests.push(test),
                    test::RunIgnored::No => {
                        if test.desc.ignore {
                            state.ignored += 1;
                        } else {
                            tests.push(test);
                        }
                    }
                    test::RunIgnored::Only => panic!("Running only ignored tests is not supported"),
                }
            } else {
                state.filtered_out += 1;
            }
        }
    }

    let n_workers = opts
        .test_threads
        .unwrap_or_else(|| thread::available_parallelism().unwrap().get());

    let max_name_len = tests
        .iter()
        .max_by_key(|t| len_if_padded(*t))
        .map(|t| t.desc.name.as_slice().len())
        .unwrap_or(0);

    let mut formatter: Box<dyn OutputFormatter + Send> = match opts.format {
        OutputFormat::Pretty => Box::new(PrettyFormatter::new(
            OutputLocation::Raw(stdout()),
            max_name_len,
            n_workers != 1,
            opts.time_options,
        )),
        OutputFormat::Junit => Box::new(JunitFormatter::new(OutputLocation::Raw(stdout()))),
        OutputFormat::Terse => Box::new(TerseFormatter::new(
            OutputLocation::Raw(stdout()),
            false,
            max_name_len,
            n_workers != 1,
        )),
        OutputFormat::Json => Box::new(JsonFormatter::new(OutputLocation::Raw(stdout()))),
    };

    formatter.write_run_start(tests.len(), None).unwrap();

    let (mut formatter, mut state) = {
        #[cfg(unix)]
        {
            execute_tests_unix(opts, n_workers, tests, formatter, state)
        }

        #[cfg(not(unix))]
        {
            execute_tests_single_threaded(opts, tests, formatter, state)
        }
    };

    state.exec_time = start_time.map(|t| TestSuiteExecTime(t.elapsed()));
    let is_success = formatter.write_run_finish(&state).unwrap();

    if !is_success {
        process::exit(ERROR_EXIT_CODE);
    }
}

#[cfg(unix)]
fn execute_tests_unix(
    opts: TestOpts,
    n_workers: usize,
    tests: Vec<TestDescAndFn>,
    formatter: Box<dyn OutputFormatter + Send>,
    state: ConsoleTestState,
) -> (Box<dyn OutputFormatter + Send>, ConsoleTestState) {
    use std::process::exit;

    use crate::util::install_kill_hook;
    use fork::{fork, Fork};
    use threadpool::ThreadPool;

    if n_workers > 1 {
        let formatter = Arc::new(Mutex::new(formatter));
        let state = Arc::new(Mutex::new(state));

        let pool = ThreadPool::with_name("rustyrts_test_thread".to_string(), n_workers);

        for test in tests {
            let state = state.clone();
            let formatter = formatter.clone();

            pool.execute(move || {
                formatter
                    .lock()
                    .unwrap()
                    .write_test_start(&test.desc)
                    .unwrap();

                let completed_test = {
                    let (mut rx, mut tx) = create_pipes().unwrap();

                    match fork().expect("Fork failed") {
                        Fork::Parent(child) => {
                            drop(tx);

                            let maybe_result = rx.recv();

                            match waitpid_wrapper(child) {
                                Ok(exit) if exit == UNUSUAL_EXIT_CODE => match maybe_result {
                                    Ok(t) => t,
                                    Err(e) => CompletedTest::failed(format!(
                                        "Failed to receive test result: {}",
                                        e
                                    )),
                                },
                                Ok(exit) => {
                                    CompletedTest::failed(format!("Wrong exit code: {}", exit))
                                }
                                Err(cause) => CompletedTest::failed(cause),
                            }
                        }
                        Fork::Child => {
                            drop(rx);
                            install_kill_hook();

                            unsafe {
                                close(std::io::stdout().as_raw_fd());
                                close(std::io::stderr().as_raw_fd());
                            }

                            let completed_test = run_test(
                                &test,
                                opts.nocapture,
                                opts.time_options.is_some(),
                                opts.time_options,
                            );
                            tx.send(completed_test).unwrap();
                            exit(UNUSUAL_EXIT_CODE);
                        }
                    }
                };

                let exec_time = completed_test.time.map(|time| TestExecTime(time));
                formatter
                    .lock()
                    .unwrap()
                    .write_result(
                        &test.desc,
                        &completed_test.result,
                        exec_time.as_ref(),
                        &completed_test.stdout,
                        &state.lock().unwrap(),
                    )
                    .unwrap();
                completed_test.evaluate_result(
                    test.desc,
                    exec_time.as_ref(),
                    &mut state.lock().unwrap(),
                );
            });
        }

        pool.join();

        let formatter = unsafe {
            Arc::<_>::try_unwrap(formatter)
                .unwrap_unchecked()
                .into_inner()
                .unwrap()
        };
        let state = unsafe {
            Arc::<_>::try_unwrap(state)
                .unwrap_unchecked()
                .into_inner()
                .unwrap()
        };

        (formatter, state)
    } else {
        execute_tests_single_threaded(opts, tests, formatter, state)
    }
}

fn execute_tests_single_threaded(
    opts: TestOpts,
    tests: Vec<TestDescAndFn>,
    mut formatter: Box<dyn OutputFormatter + Send>,
    mut state: ConsoleTestState,
) -> (Box<dyn OutputFormatter + Send>, ConsoleTestState) {
    for test in tests {
        formatter.write_test_start(&test.desc).unwrap();

        let completed_test = run_test(
            &test,
            opts.nocapture,
            opts.time_options.is_some(),
            opts.time_options,
        );

        let exec_time = completed_test.time.map(|time| TestExecTime(time));
        formatter
            .write_result(
                &test.desc,
                &completed_test.result,
                exec_time.as_ref(),
                &completed_test.stdout,
                &state,
            )
            .unwrap();

        completed_test.evaluate_result(test.desc, exec_time.as_ref(), &mut state);
    }
    (formatter, state)
}

fn run_test(
    test: &test::TestDescAndFn,
    nocapture: bool,
    report_time: bool,
    time_opts: Option<TestTimeOptions>,
) -> CompletedTest {
    let data = Arc::new(Mutex::new(Vec::new()));
    if !nocapture {
        io::set_output_capture(Some(data.clone()));
    }
    let (result, time) = match test.testfn {
        test::TestFn::StaticTestFn(f) => {
            let start = report_time.then(Instant::now);
            let result = catch_unwind(AssertUnwindSafe(|| {
                f().unwrap();
            }));
            let time = start.map(|start| start.elapsed());
            let exec_time = time.map(|time| TestExecTime(time));

            let test_result = match result {
                Ok(()) => calc_result(&test.desc, Ok(()), &time_opts, &exec_time),
                Err(e) => calc_result(&test.desc, Err(e.as_ref()), &time_opts, &exec_time),
            };

            (test_result, time)
        }
        _ => unimplemented!("Only StaticTestFns are supported right now"),
    };
    io::set_output_capture(None);
    let stdout = data.lock().unwrap_or_else(|e| e.into_inner()).to_vec();

    CompletedTest {
        result,
        stdout,
        time,
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq)]
pub(crate) struct CompletedTest {
    result: TestResult,
    stdout: Vec<u8>,
    time: Option<Duration>,
}

impl CompletedTest {
    #[cfg(unix)]
    fn failed(cause: String) -> Self {
        CompletedTest {
            result: TestResult::TrFailedMsg(cause),
            stdout: Vec::default(),
            time: None,
        }
    }

    fn evaluate_result(
        self,
        desc: TestDesc,
        exec_time: Option<&TestExecTime>,
        state: &mut ConsoleTestState,
    ) {
        state
            .write_log_result(&desc, &self.result, exec_time)
            .unwrap();
        match self.result {
            TestResult::TrOk => state.passed += 1,
            TestResult::TrFailedMsg(_) | TestResult::TrFailed => {
                state.failed += 1;
                state.failures.push((desc, self.stdout))
            }
            TestResult::TrIgnored => {
                state.ignored += 1;
                state.ignores.push((desc, self.stdout));
            }
            TestResult::TrTimedFail => {
                state.failed += 1;
                state.time_failures.push((desc, self.stdout))
            }
        }
    }
}
