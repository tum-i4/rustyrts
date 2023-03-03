use ipc_channel::ipc;
use nix::sys::wait::waitpid;
use nix::sys::wait::WaitPidFlag;
use nix::unistd::{fork, ForkResult};
use std::any::Any;
use std::io::{self, Write};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::process::exit;
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::Duration;
use test::{StaticTestFn, TestDescAndFn};
use threadpool::ThreadPool;

use crate::util::{get_affected_path, get_dynamic_path, read_lines};

//######################################################################################################################
// This file is inspired by coppers
// Source: https://github.com/ThijsRay/coppers/blob/add2a7d56301137b339d8d791e1426ceb9022c7e/src/test_runner/mod.rs

// Modifications:
// * removed sensor-related code
// * removed repetition of tests
// * use threadpool to execute tests in parallel
// * fork for every single test

#[no_mangle]
pub fn rustyrts_runner(tests: &[&test::TestDescAndFn]) {
    let project_dir = std::env::var("PROJECT_DIR").unwrap();
    let path_buf = get_affected_path(get_dynamic_path(&project_dir));
    let affected_tests = Arc::new(RwLock::new(read_lines(path_buf)));

    let all_tests: Vec<_> = tests.iter().map(make_owned_test).collect();
    let mut tests: Vec<TestDescAndFn> = Vec::new();

    let mut filtered: i32 = 0;
    for test in all_tests {
        if affected_tests
            .read()
            .unwrap()
            .contains(test.desc.name.as_slice())
        {
            tests.push(test);
        } else {
            filtered += 1;
        }
    }

    println!("\nRunning {} tests", tests.len());

    let ignored: Arc<Mutex<i32>> = Arc::new(Mutex::new(0));

    let passed_tests: Arc<Mutex<Vec<CompletedTest>>> = Arc::new(Mutex::new(Vec::new()));
    let failed_tests: Arc<Mutex<Vec<CompletedTest>>> = Arc::new(Mutex::new(Vec::new()));

    let n_workers = thread::available_parallelism().unwrap().get();
    let pool = ThreadPool::new(n_workers);

    for test in tests {
        let passed_tests = passed_tests.clone();
        let failed_tests = failed_tests.clone();
        let ignored = ignored.clone();

        let affected_tests = affected_tests.clone();

        pool.execute(move || {
            if affected_tests
                .read()
                .unwrap()
                .contains(test.desc.name.as_slice())
            {
                let result = {
                    let (tx, rx) = ipc::channel().unwrap();

                    match unsafe { fork() } {
                        Ok(ForkResult::Parent { child, .. }) => {
                            let name = test.desc.name.as_slice().to_string();

                            //println!("Waiting for {}", name);

                            waitpid(child, Some(WaitPidFlag::__WALL)).expect("waitpid() failed");

                            //println!("Completed {}", name);

                            let maybe_result = rx.try_recv_timeout(Duration::ZERO);

                            //println!("Received result for {}: {:?}", name, maybe_result);

                            let result: CompletedTest =
                                maybe_result.unwrap_or(CompletedTest::failed(name));
                            result
                        }
                        Ok(ForkResult::Child) => {
                            let result = run_test(test);
                            tx.send(result).unwrap();
                            exit(0);
                        }
                        Err(_) => panic!("Fork failed"),
                    }
                };

                print_test_result(&result);

                match result.state {
                    TestResult::Passed => passed_tests.lock().unwrap().push(result),
                    TestResult::Failed(_) => failed_tests.lock().unwrap().push(result),
                    TestResult::Ignored => *ignored.lock().unwrap() += 1,
                }
            }
        });
    }

    pool.join();

    let failed_tests = Arc::<Mutex<Vec<CompletedTest>>>::try_unwrap(failed_tests)
        .unwrap()
        .into_inner()
        .unwrap();
    let passed_tests = Arc::<Mutex<Vec<CompletedTest>>>::try_unwrap(passed_tests)
        .unwrap()
        .into_inner()
        .unwrap();
    let ignored = Arc::<Mutex<i32>>::try_unwrap(ignored)
        .unwrap()
        .into_inner()
        .unwrap();

    print_failures(&failed_tests).unwrap();

    println!(
        "test result: {}. {} passed; {} failed; {ignored} ignored; {filtered} filtered;",
        passed(failed_tests.is_empty()),
        passed_tests.len(),
        failed_tests.len()
    );
}

fn run_test(test: test::TestDescAndFn) -> CompletedTest {
    // If a test is marked with #[ignore], it should not be executed
    if test.desc.ignore {
        CompletedTest::empty(test.desc.name.to_string())
    } else {
        // Use internal compiler function `set_output_capture` to capture the output of the
        // tests.
        let data = Arc::new(Mutex::new(Vec::new()));
        io::set_output_capture(Some(data.clone()));

        //let mut us = 0;

        let state = match test.testfn {
            test::TestFn::StaticTestFn(f) => {
                let result = catch_unwind(AssertUnwindSafe(|| {
                    f();
                }));
                //us += sensor.get_elapsed_time_us();

                test_state(&test.desc, result)
            }
            _ => unimplemented!("Only StaticTestFns are supported right now"),
        };

        // Reset the output capturing to the default behavior and transform the captured output
        // to a vector of bytes.
        io::set_output_capture(None);
        let stdout = Some(data.lock().unwrap_or_else(|e| e.into_inner()).to_vec());

        CompletedTest {
            name: test.desc.name.to_string(),
            state,
            //us: Some(us),
            stdout,
        }
    }
}

fn print_failures(tests: &Vec<CompletedTest>) -> std::io::Result<()> {
    if !tests.is_empty() {
        let stdout = io::stdout();
        let mut handle = stdout.lock();
        for test in tests {
            if let Some(captured) = &test.stdout {
                handle.write_fmt(format_args!("\n---- {} stdout ----\n", test.name))?;
                handle.write_all(captured)?;
                handle.write_all(b"\n")?;
            }
        }
        handle.write_all(b"\nfailures:\n")?;
        for test in tests {
            handle.write_fmt(format_args!("\t{}", test.name))?;
            if let TestResult::Failed(Some(msg)) = &test.state {
                handle.write_fmt(format_args!(": {}\n", msg))?;
            }
        }
        handle.write_all(b"\n")?;
    }
    Ok(())
}

fn print_test_result(test: &CompletedTest) {
    match test.state {
        TestResult::Passed => {
            //            let us = test.us.unwrap();
            println!(
                "test {} ... {}", // - [in {us} μs]",
                test.name,
                passed(true)
            )
        }
        TestResult::Failed(_) => {
            println!("test {} ... {}", test.name, passed(false))
        }
        _ => {}
    }
}

fn passed(condition: bool) -> &'static str {
    if condition {
        "ok"
    } else {
        "FAILED"
    }
}

fn make_owned_test(test: &&TestDescAndFn) -> TestDescAndFn {
    match test.testfn {
        StaticTestFn(f) => TestDescAndFn {
            testfn: StaticTestFn(f),
            desc: test.desc.clone(),
        },
        _ => panic!("non-static tests passed to test::test_main_static"),
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq)]
enum TestResult {
    Passed,
    Failed(Option<String>),
    Ignored,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq)]
pub(crate) struct CompletedTest {
    name: String,
    state: TestResult,
    //    us: Option<u128>,
    stdout: Option<Vec<u8>>,
}

impl CompletedTest {
    fn empty(name: String) -> Self {
        CompletedTest {
            name,
            state: TestResult::Ignored,
            //us: None,
            stdout: None,
        }
    }

    fn failed(name: String) -> Self {
        CompletedTest {
            name,
            state: TestResult::Failed(Some("Test aborted unexpectedly".to_string())),
            //us: None,
            stdout: None,
        }
    }
}

fn test_state(desc: &test::TestDesc, result: Result<(), Box<dyn Any + Send>>) -> TestResult {
    use test::ShouldPanic;

    let result = match (desc.should_panic, result) {
        (ShouldPanic::No, Ok(())) | (ShouldPanic::Yes, Err(_)) => TestResult::Passed,
        (ShouldPanic::YesWithMessage(msg), Err(ref err)) => {
            let maybe_panic_str = err
                .downcast_ref::<String>()
                .map(|e| &**e)
                .or_else(|| err.downcast_ref::<&'static str>().copied());

            if maybe_panic_str.map(|e| e.contains(msg)).unwrap_or(false) {
                TestResult::Passed
            } else if let Some(panic_str) = maybe_panic_str {
                TestResult::Failed(Some(format!(
                    r#"panic did not contain expected string
      panic message: `{:?}`,
 expected substring: `{:?}`"#,
                    panic_str, msg
                )))
            } else {
                TestResult::Failed(Some(format!(
                    r#"expected panic with string value,
 found non-string value: `{:?}`
     expected substring: `{:?}`"#,
                    (**err).type_id(),
                    msg
                )))
            }
        }
        (ShouldPanic::Yes, Ok(())) | (ShouldPanic::YesWithMessage(_), Ok(())) => {
            TestResult::Failed(Some("test did not panic as expected".to_string()))
        }
        _ => TestResult::Failed(None),
    };

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::panic;
    use test::TestDesc;

    fn default_test_desc() -> TestDesc {
        TestDesc {
            name: test::StaticTestName("Test"),
            ignore: false,
            ignore_message: None,
            should_panic: test::ShouldPanic::No,
            compile_fail: false,
            no_run: false,
            test_type: test::TestType::UnitTest,
        }
    }

    fn generate_panic_info(message: &'static str) -> Box<dyn Any + Send> {
        catch_unwind(|| {
            panic::panic_any(message);
        })
        .unwrap_err()
    }

    #[test]
    fn test_succeeded_succeeds_without_panic() {
        let desc = default_test_desc();
        let result = Ok(());
        assert_eq!(test_state(&desc, result), TestResult::Passed)
    }

    #[test]
    fn test_succeeded_unexpected_panic() {
        let desc = default_test_desc();
        let panic_str = "Assertion failed";
        let result = Err(generate_panic_info(panic_str));
        let test_result = test_state(&desc, result);
        if TestResult::Failed(None) != test_result {
            panic!("Result was {:?}", test_result)
        }
    }

    #[test]
    fn test_succeeded_expected_panic_and_did_panic() {
        let mut desc = default_test_desc();
        desc.should_panic = test::ShouldPanic::Yes;
        let result = Err(generate_panic_info("Assertion failed"));
        let test_result = test_state(&desc, result);
        assert_eq!(test_result, TestResult::Passed)
    }

    #[test]
    fn test_succeeded_expected_panic_but_did_not_panic() {
        let mut desc = default_test_desc();
        desc.should_panic = test::ShouldPanic::Yes;
        let result = Ok(());
        let test_result = test_state(&desc, result);
        match test_result {
            TestResult::Failed(Some(msg)) => assert!(msg.contains("test did not panic")),
            _ => panic!("Result was {:?}", test_result),
        }
    }

    #[test]
    fn test_succeeded_expected_panic_with_str_message() {
        let mut desc = default_test_desc();
        desc.should_panic = test::ShouldPanic::YesWithMessage("This is a message");
        let result = Err(generate_panic_info("This is a message"));
        assert_eq!(test_state(&desc, result), TestResult::Passed)
    }

    #[test]
    fn test_succeeded_expected_panic_with_string_message() {
        let mut desc = default_test_desc();
        desc.should_panic = test::ShouldPanic::YesWithMessage("This is a message");
        let result = Err(catch_unwind(|| {
            panic::panic_any(String::from("This is a message"));
        })
        .unwrap_err());
        assert_eq!(test_state(&desc, result), TestResult::Passed)
    }

    #[test]
    fn test_succeeded_expected_panic_with_string_message_but_got_no_string() {
        let mut desc = default_test_desc();
        desc.should_panic = test::ShouldPanic::YesWithMessage("This is a message");
        let result = Err(catch_unwind(|| {
            panic::panic_any(123);
        })
        .unwrap_err());
        let test_result = test_state(&desc, result);
        match test_result {
            TestResult::Failed(Some(msg)) => {
                assert!(msg.contains("expected panic with string value"))
            }
            _ => panic!("Result is {:?}", test_result),
        }
    }

    #[test]
    fn test_succeeded_expected_panic_with_wrong_message() {
        let mut desc = default_test_desc();
        desc.should_panic = test::ShouldPanic::YesWithMessage("This is a message");
        let result = Err(generate_panic_info("This is another message"));
        let test_result = test_state(&desc, result);
        match test_result {
            TestResult::Failed(Some(msg)) => {
                assert!(msg.contains("panic did not contain expected string"))
            }
            _ => panic!("Result is {:?}", test_result),
        }
    }

    #[test]
    fn test_succeeded_expected_panic_with_message_but_with_no_message() {
        let mut desc = default_test_desc();
        desc.should_panic = test::ShouldPanic::YesWithMessage("This is a message");
        let result = Err(generate_panic_info(""));
        let test_result = test_state(&desc, result);
        match test_result {
            TestResult::Failed(Some(msg)) => {
                assert!(msg.contains("panic did not contain expected string"))
            }
            _ => panic!("Result is {:?}", test_result),
        }
    }

    #[test]
    fn test_succeeded_expected_panic_with_message_but_did_not_panic() {
        let mut desc = default_test_desc();
        desc.should_panic = test::ShouldPanic::YesWithMessage("This is a message");
        let result = Ok(());
        let test_result = test_state(&desc, result);
        match test_result {
            TestResult::Failed(Some(msg)) => {
                assert!(msg.contains("test did not panic as expected"))
            }
            _ => panic!("Result is {:?}", test_result),
        }
    }
}
