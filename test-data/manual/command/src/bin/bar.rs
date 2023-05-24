use command::library_fn;
use std::process::ExitCode;

#[cfg(unix)]
pub fn main() -> ExitCode {
    use std::{
        path::PathBuf,
        process::{Command, ExitCode},
    };

    let path = PathBuf::from("target/debug/foo");

    let status = Command::new(path).status().unwrap();
    return ExitCode::from(status.code().unwrap() as u8);
}
