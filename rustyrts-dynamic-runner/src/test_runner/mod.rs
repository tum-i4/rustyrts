use nix::libc::write;
use nix::sys::wait::waitpid;
use nix::unistd::{dup2, execvp, fork, pipe2, ForkResult, Pid};
use std::any::Any;
use std::fs::{self, read, read_to_string, File};
use std::io::{self, Read, Write};
use std::os::fd::FromRawFd;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::process::{exit, Command};
use std::sync::{Arc, Mutex};
use test::{StaticTestFn, TestDescAndFn};

//######################################################################################################################
// This file is inspired by coppers
// Source: https://github.com/ThijsRay/coppers/blob/add2a7d56301137b339d8d791e1426ceb9022c7e/src/test_runner/mod.rs

// Modifications:
// * removed sensor-related code
// * removed repetition of tests

#[no_mangle]
pub fn runner(tests: &[&test::TestDescAndFn]) {
    let tests: Vec<_> = tests.iter().map(make_owned_test).collect();

    println!("\nRunning {} tests", tests.len());

    let mut ignored = 0;
    //let mut filtered = 0;

    let mut passed_tests = Vec::new();
    let mut failed_tests = Vec::new();

    for test in tests {
        let (p_out, p_in) = pipe2(nix::fcntl::OFlag::empty()).unwrap();

        println!("Forking for {}", test.desc.name);

        match unsafe { fork() } {
            Ok(ForkResult::Parent { child, .. }) => {
                let mut f_out = unsafe { File::from_raw_fd(p_out) };

                waitpid(child, None).expect("waitpid() failed");

                println!("Waited for {}", test.desc.name);

                let mut result_str = String::new();
                f_out.read_to_string(&mut result_str).unwrap();

                println!("Result: {}", result_str);

                let result: CompletedTest = serde_json::from_str(&result_str).unwrap();

                match result.state {
                    TestResult::Passed => {
                        passed_tests.push(result);
                    }
                    TestResult::Failed(_) => failed_tests.push(result),
                    TestResult::Ignored => ignored += 1,
                    //TestResult::Filtered => filtered += 1,
                }
            }
            Ok(ForkResult::Child) => {
                let result = run_test(test);
                print_test_result(&result);

                let result_json = serde_json::to_string(&result).unwrap();
                {
                    let mut f_in = unsafe { File::from_raw_fd(p_in) };
                    write!(&mut f_in, "{}", result_json).unwrap();

                    println!("Wrote result");
                }
                exit(0);
            }
            Err(_) => println!("Fork failed"),
        }
    }

    print_failures(&failed_tests).unwrap();

    println!(
        "test result: {}.\t{} passed;\t{} failed;\t{ignored} ignored;",
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
    // TODO: add Filtered
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
