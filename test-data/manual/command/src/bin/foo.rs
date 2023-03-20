use std::process::ExitCode;

use command::library_fn;

#[cfg(unix)]
pub fn main() -> ExitCode {
    return ExitCode::from(library_fn());
}
