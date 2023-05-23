#![feature(rustc_private)]

extern crate rustc_driver;
extern crate rustc_log;
extern crate rustc_session;

use rustc_session::config::ErrorOutputType;
use rustc_session::early_error;
use rustyrts::callbacks_shared::export_checksums_and_changes;
use rustyrts::constants::ENV_PROJECT_DIR;
use rustyrts::format::create_logger;
use rustyrts::static_rts::callback::StaticRTSCallbacks;
use rustyrts::utils;
use std::env;
use std::process;

//######################################################################################################################
// This file is heavily inspired by rust-mir-checker
// Source: https://github.com/lizhuohua/rust-mir-checker/blob/86c3c26e797d3e25a38044fa98b765c5d220e4ea/src/bin/mir-checker.rs
//######################################################################################################################

/// Exit status code used for successful compilation and help output.
pub const EXIT_SUCCESS: i32 = 0;

/// Exit status code used for compilation failures and invalid flags.
pub const EXIT_FAILURE: i32 = 1;

fn main() {
    rustc_log::init_rustc_env_logger().unwrap();
    create_logger().init();

    let result = rustc_driver::catch_fatal_errors(move || {
        let mut rustc_args = env::args_os()
            .enumerate()
            .map(|(i, arg)| {
                arg.into_string().unwrap_or_else(|arg| {
                    early_error(
                        ErrorOutputType::default(),
                        &format!("Argument {} is not valid Unicode: {:?}", i, arg),
                    )
                })
            })
            .collect::<Vec<_>>();

        if let Some(sysroot) = utils::compile_time_sysroot() {
            let sysroot_flag = "--sysroot";
            if !rustc_args.iter().any(|e| e == sysroot_flag) {
                // We need to overwrite the default that librustc would compute.
                rustc_args.push(sysroot_flag.to_owned());
                rustc_args.push(sysroot);
            }
        }

        rustc_args.push("--cap-lints".to_string());
        rustc_args.push("allow".to_string());

        let source_path = env::var(ENV_PROJECT_DIR).unwrap();
        let mut callbacks = StaticRTSCallbacks::new(source_path);

        let run_compiler = rustc_driver::RunCompiler::new(&rustc_args, &mut callbacks);
        run_compiler.run()
    });

    let exit_code = match result {
        Ok(_) => {
            export_checksums_and_changes();
            EXIT_SUCCESS
        }
        Err(_) => EXIT_FAILURE,
    };

    process::exit(exit_code);
}
