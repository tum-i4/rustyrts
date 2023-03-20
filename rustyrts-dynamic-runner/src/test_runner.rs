use fork::{fork, Fork};
use std::io::{self, stdout};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::process::{self, exit};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use test::test::{parse_opts, TestExecTime, TestTimeOptions};
use test::{OutputFormat, ShouldPanic, TestDesc, TestDescAndFn, TestOpts};
use threadpool::ThreadPool;

use crate::libtest::{
    calc_result, len_if_padded, make_owned_test, ConsoleTestState, JsonFormatter, OutputFormatter,
    OutputLocation, PrettyFormatter, TestResult, TestSuiteExecTime, ERROR_EXIT_CODE,
};

#[cfg(target_family = "unix")]
use libc::c_int;

#[cfg(target_family = "unix")]
use crate::pipe::create_pipes;

#[cfg(target_family = "unix")]
use crate::util::waitpid_wrapper;

const UNUSUAL_EXIT_CODE: c_int = 15;

#[no_mangle]
pub fn rustyrts_runner(tests: &[&test::TestDescAndFn]) {
    let args: &Vec<String> = &std::env::args().collect::<Vec<_>>();
    let opts: TestOpts = parse_opts(args).unwrap().unwrap();

    // Exclude some options that just do not fit into RTS
    assert!(opts.run_tests);
    assert!(opts.filter_exact);
    assert!(!opts.fail_fast);
    assert!(!opts.bench_benchmarks);
    assert!(opts.skip.is_empty());
    assert!(!opts.shuffle);

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

    let formatter: Box<dyn OutputFormatter + Send> = match opts.format {
        OutputFormat::Pretty => {
            let max_name_len = tests
                .iter()
                .max_by_key(|t| len_if_padded(*t))
                .map(|t| t.desc.name.as_slice().len())
                .unwrap_or(0);

            Box::new(PrettyFormatter::new(
                OutputLocation::Raw(stdout()),
                max_name_len,
                n_workers != 1,
                opts.time_options,
            ))
        }
        OutputFormat::Json => Box::new(JsonFormatter::new(OutputLocation::Raw(stdout()))),
        OutputFormat::Terse => todo!(),
        OutputFormat::Junit => todo!(),
    };

    let (mut formatter, mut state) = if cfg!(unix) {
        execute_tests_unix(opts, n_workers, tests, formatter, state)
    } else {
        execute_tests_single_threaded(opts, tests, formatter, state)
    };

    state.exec_time = start_time.map(|t| TestSuiteExecTime(t.elapsed()));
    let is_success = formatter.write_run_finish(&state).unwrap();

    if !is_success {
        process::exit(ERROR_EXIT_CODE);
    }
}

#[cfg(target_family = "unix")]
fn execute_tests_unix(
    opts: TestOpts,
    n_workers: usize,
    tests: Vec<TestDescAndFn>,
    formatter: Box<dyn OutputFormatter + Send>,
    state: ConsoleTestState,
) -> (Box<dyn OutputFormatter + Send>, ConsoleTestState) {
    if n_workers > 1 {
        let formatter = Arc::new(Mutex::new(formatter));

        let state = Arc::new(Mutex::new(state));

        formatter
            .lock()
            .unwrap()
            .write_run_start(tests.len(), None)
            .unwrap();

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
    formatter.write_run_start(tests.len(), None).unwrap();

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
            TestResult::TrIgnored => state.ignored += 1,
            TestResult::TrTimedFail => {
                state.failed += 1;
                state.time_failures.push((desc, self.stdout))
            }
        }
    }
}
