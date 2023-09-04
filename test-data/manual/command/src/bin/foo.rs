#![feature(try_trait_v2)]
use std::process::ExitCode;

use command::library_fn;

#[cfg(unix)]
pub fn main() -> ExitCode {
    use std::ops::{ControlFlow, FromResidual};

    library_fn();
    library_fn();
    library_fn();
    library_fn();

    assert_eq!(
        ControlFlow::<_, String>::from_residual(ControlFlow::Break(5)),
        ControlFlow::Break(5),
    );

    return ExitCode::from(library_fn());
}
