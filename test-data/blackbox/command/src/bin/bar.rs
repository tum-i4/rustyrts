use command::library_fn;
use std::process::ExitCode;

#[cfg(unix)]
pub fn main() -> ExitCode {
    use std::{
        path::PathBuf,
        process::{Command, ExitCode},
    };

    let mut path =
        PathBuf::from(std::env::var("CARGO_TARGET_DIR").unwrap_or_else(|_| "target".to_string()));
    path.push("debug");
    path.push("foo");

    let status = Command::new(path).status().unwrap();
    return ExitCode::from(status.code().unwrap() as u8);
}
