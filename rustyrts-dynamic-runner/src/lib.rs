#![feature(test)]
#![feature(internal_output_capture)]

extern crate test;

mod fifo;
mod test_runner;
mod util;

pub use crate::test_runner::rustyrts_runner;
