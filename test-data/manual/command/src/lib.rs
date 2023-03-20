use std::process::Command;

pub fn library_fn() -> u8 {
    42
}

#[test]
fn test() {
    let mut path = std::env::current_exe().unwrap();

    path.pop();
    path.pop();
    path.push("foo");

    let status = Command::new(path).status().unwrap();
    assert_eq!(status.code().unwrap(), 42);
}
