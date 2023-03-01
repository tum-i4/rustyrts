#![feature(test)]
#![feature(internal_output_capture)]

extern crate test;

mod test_runner;
mod util;

pub use crate::test_runner::runner;
