#![feature(rustc_private)]

extern crate rustc_driver;
extern crate rustc_log;

use rustyrts::{
    constants::ENV_ONLY_INSTRUMENTATION,
    dynamic_rts::callback::{DynamicRTSCallbacks, InstrumentingRTSCallbacks},
};
use rustyrts::{constants::ENV_TARGET_DIR, main_rustyrts};
use std::path::PathBuf;

fn main() {
    if std::env::var(ENV_ONLY_INSTRUMENTATION).is_ok() {
        let callbacks = InstrumentingRTSCallbacks::new();
        main_rustyrts(callbacks, false);
    } else {
        let target_dir = PathBuf::from(std::env::var(ENV_TARGET_DIR).unwrap());
        let callbacks = DynamicRTSCallbacks::new(target_dir);
        main_rustyrts(callbacks, false);
    };
}
