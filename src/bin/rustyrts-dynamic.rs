#![feature(rustc_private)]

extern crate rustc_ast_pretty;
extern crate rustc_driver;
extern crate rustc_error_codes;
extern crate rustc_errors;
extern crate rustc_hash;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_log;
extern crate rustc_middle;
extern crate rustc_session;
extern crate rustc_span;

use rustc_session::config::ErrorOutputType;
use rustc_session::early_error;
use rustyrts::callbacks_shared::export_checksums_and_changes;
use rustyrts::constants::{ENV_SKIP_ANALYSIS, ENV_TARGET_DIR};
use rustyrts::dynamic_rts::callback::DynamicRTSCallbacks;
use rustyrts::format::create_logger;
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

    let skip = env::var(ENV_SKIP_ANALYSIS).is_ok()
        && !(env::var(ENV_TARGET_DIR).map(|var| var.ends_with("trybuild")) == Ok(true));

    if !skip {
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

            // Provide information on where to find rustyrts-dynamic-rlib
            let cargo_home = std::env::var("CARGO_HOME").unwrap_or("~/.cargo".to_string());

            rustc_args.push("-L".to_string());
            rustc_args.push(format!("{}/bin", cargo_home).to_string());

            rustc_args.push("--cap-lints".to_string());
            rustc_args.push("allow".to_string());

            if let Some(sysroot) = utils::compile_time_sysroot() {
                let sysroot_flag = "--sysroot";
                if !rustc_args.iter().any(|e| e == sysroot_flag) {
                    // We need to overwrite the default that librustc would compute.
                    rustc_args.push(sysroot_flag.to_owned());
                    rustc_args.push(sysroot);
                }
            }

            let mut callbacks = DynamicRTSCallbacks::new();

            let run_compiler = rustc_driver::RunCompiler::new(&rustc_args, &mut callbacks);
            run_compiler.run()
        });

        let result = result.unwrap();
        let exit_code = match result {
            Ok(_) => {
                export_checksums_and_changes(false);
                EXIT_SUCCESS
            }
            Err(_) => EXIT_FAILURE,
        };

        process::exit(exit_code);
    }
}
