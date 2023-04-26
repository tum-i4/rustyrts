use std::{path::PathBuf, process::Command};

#[test]
fn test_foo() {
    let exe = env!("CARGO_BIN_EXE_foo");
    let path = PathBuf::from(exe);

    let status = Command::new(path).status().unwrap();
    assert_eq!(status.code().unwrap(), 42);
}

#[test]
fn test_bar() {
    let exe = env!("CARGO_BIN_EXE_bar");
    let path = PathBuf::from(exe);

    let status = Command::new(path).status().unwrap();
    assert_eq!(status.code().unwrap(), 42);
}
