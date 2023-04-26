use std::process::ExitCode;

use command::library_fn;

#[cfg(unix)]
pub fn main() -> ExitCode {
    library_fn();
    library_fn();
    library_fn();
    library_fn();

    return ExitCode::from(library_fn());
}
