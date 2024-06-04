#![feature(test)]
extern crate test;

use test::{test::parse_opts, test_main_static, test_main_static_abort, TestOpts};

const ERROR_EXIT_CODE: i32 = 101;

#[no_mangle]
pub fn rustyrts_runner(tests: &[&test::TestDescAndFn]) {
    let args = std::env::args().collect::<Vec<_>>();

    let opts: TestOpts = match parse_opts(&args) {
        Some(Ok(o)) => o,
        Some(Err(msg)) => {
            eprintln!("error: {msg}");
            std::process::exit(ERROR_EXIT_CODE)
        }
        None => return,
    };

    let is_multithreaded = opts.test_threads.map_or(true, |t| t > 1);

    if !is_multithreaded {
        test_main_static(tests);
    } else {
        // When panic=abort is set (which would normally lead to a call to test_main_static_abort), a separate process is
        // forked for every test case.
        // The panic hook gets wrapped in pre_test such that traces are written even if the test panics
        test_main_static_abort(tests);
    }
}
