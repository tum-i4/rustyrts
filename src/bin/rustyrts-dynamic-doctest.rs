use rustyrts::{
    constants::{ENV_ONLY_INSTRUMENTATION, ENV_TARGET_DIR},
    dynamic_rts::callback_doctest::AnalyzingRTSCallbacks,
};
use rustyrts::{dynamic_rts::callback_doctest::InstrumentingDoctestRTSCallbacks, main_rustyrts};
use std::path::PathBuf;

fn main() {
    if std::env::var(ENV_ONLY_INSTRUMENTATION).is_ok() {
        let callbacks = InstrumentingDoctestRTSCallbacks::new();
        main_rustyrts(callbacks, false);
    } else {
        let target_dir = PathBuf::from(std::env::var(ENV_TARGET_DIR).unwrap());
        let callbacks = AnalyzingRTSCallbacks::new(target_dir);
        main_rustyrts(callbacks, true);
    };
}
