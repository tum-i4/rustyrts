//########
// The purpose of this crate is, to ensure that the test-runner used in dynamic RustyRTS is able to handle certain properties of tests

use std::process::abort;

fn main() {
    println!("Hello, world!");
}

// 1. Test that is ignored
// No traces will be collected for this test
#[test]
#[ignore]
pub fn test_ignored() {
    main();
    assert!(false)
}

// 2. Succeeding test
#[test]
pub fn test_success() {
    main();
    assert!(true)
}

// 3. Failing test (panicking, unwinding)
#[test]
pub fn test_failed() {
    main();
    assert!(false)
}

// 4. Failing test (panicking, aborting)
// No traces will be collected for this test
#[test]
pub fn test_segfault() {
    main();
    abort()
}
