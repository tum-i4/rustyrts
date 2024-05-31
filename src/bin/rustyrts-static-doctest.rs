use rustyrts::main_rustyrts;
use rustyrts::{
    constants::ENV_TARGET_DIR, static_rts::callback_doctest::StaticDoctestRTSCallbacks,
};
use std::path::PathBuf;

fn main() {
    let target_dir = PathBuf::from(std::env::var(ENV_TARGET_DIR).unwrap());
    let callbacks = StaticDoctestRTSCallbacks::new(target_dir);
    main_rustyrts(callbacks, true);
}
