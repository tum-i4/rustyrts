#![feature(rustc_private)]

extern crate rustc_driver;
extern crate rustc_log;

use rustc_log::LoggerConfig;
use rustyrts::callbacks_shared::export_checksums_and_changes;
use rustyrts::dynamic_rts::callback::DynamicRTSCallbacks;
use rustyrts::format::create_logger;
use rustyrts::{
    constants::{ENV_BLACKBOX_TEST, ENV_SKIP_ANALYSIS, ENV_TARGET_DIR},
    fs_utils::{get_cache_path, CacheKind},
};
use std::process;
use std::{env, path::PathBuf};

//######################################################################################################################
// This file is heavily inspired by rust-mir-checker
// Source: https://github.com/lizhuohua/rust-mir-checker/blob/86c3c26e797d3e25a38044fa98b765c5d220e4ea/src/bin/mir-checker.rs
//######################################################################################################################

/// Exit status code used for successful compilation and help output.
pub const EXIT_SUCCESS: i32 = 0;

/// Exit status code used for compilation failures and invalid flags.
pub const EXIT_FAILURE: i32 = 1;

fn main() {
    rustc_log::init_logger(LoggerConfig::from_env("RUSTC")).unwrap();
    create_logger().init();

    let skip = env::var(ENV_SKIP_ANALYSIS).is_ok()
        && !(env::var(ENV_TARGET_DIR).map(|var| var.ends_with("trybuild")) == Ok(true));

    if !skip {
        let result = rustc_driver::catch_fatal_errors(move || {
            let mut rustc_args = env::args_os()
                .skip(1)
                .enumerate()
                .map(|(i, arg)| {
                    arg.into_string().unwrap_or_else(|arg| {
                        eprintln!("Argument {} is not valid Unicode: {:?}", i, arg);
                        process::exit(EXIT_FAILURE);
                    })
                })
                .map(|arg| {
                    // when running blackbox tests, this ensures that stable crate ids do not change if features are enabled
                    if std::env::var(ENV_BLACKBOX_TEST).is_ok() {
                        if arg.starts_with("metadata=") {
                            return "metadata=".to_string();
                        }
                    }
                    arg
                })
                .collect::<Vec<_>>();

            // Provide information on where to find rustyrts-dynamic-rlib
            let mut rlib_source =
                PathBuf::from(std::env::var("CARGO_HOME").expect("Did not find CARGO_HOME"));
            rlib_source.push("bin");

            rustc_args.push("-L".to_string());
            rustc_args.push(rlib_source.display().to_string());

            rustc_args.push("--cap-lints".to_string());
            rustc_args.push("allow".to_string());

            let maybe_cache_path = get_cache_path(CacheKind::Dynamic);
            let mut callbacks = DynamicRTSCallbacks::new(maybe_cache_path);

            let run_compiler = rustc_driver::RunCompiler::new(&rustc_args, &mut callbacks);
            run_compiler.run()
        });

        let result = result.unwrap();
        let exit_code = match result {
            Ok(_) => {
                export_checksums_and_changes(false, false);
                EXIT_SUCCESS
            }
            Err(_) => EXIT_FAILURE,
        };

        process::exit(exit_code);
    }
}
