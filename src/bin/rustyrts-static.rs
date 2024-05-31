use rustyrts::constants::ENV_TARGET_DIR;
use rustyrts::main_rustyrts;
use rustyrts::static_rts::callback::StaticRTSCallbacks;
use std::path::PathBuf;

fn main() {
    let target_dir = PathBuf::from(std::env::var(ENV_TARGET_DIR).unwrap());
    let callbacks = StaticRTSCallbacks::new(target_dir);
    main_rustyrts(callbacks, false);
}
