#![feature(rustc_private)]
#![allow(mutable_transmutes)]
#![feature(slice_split_once)]

// required for calling the compiler and providing callbacks
extern crate rustc_driver;
extern crate rustc_driver_impl;
extern crate rustc_interface;
extern crate rustc_log;
extern crate rustc_session;

// required for hashing and computing checksums
extern crate rustc_data_structures;

// required for analyzing and modifying the MIR
extern crate rustc_abi;
extern crate rustc_ast;
extern crate rustc_ast_pretty;
extern crate rustc_attr;
extern crate rustc_error_messages;
extern crate rustc_errors;
extern crate rustc_feature;
extern crate rustc_hir;
extern crate rustc_hir_pretty;
extern crate rustc_lexer;
extern crate rustc_middle;
extern crate rustc_span;
extern crate rustc_target;

extern crate rustc_incremental;

// required by code from librustdoc
extern crate rustc_resolve;

pub mod dynamic_rts;
pub mod static_rts;

pub mod callbacks_shared;
pub mod checksums;
pub mod const_visitor;
pub mod constants;
pub mod format;
pub mod fs_utils;
pub mod info;
pub mod names;

use constants::ENV_BLACKBOX_TEST;
use format::setup_logger;
use rustc_driver_impl::Callbacks;
use rustc_log::LoggerConfig;
use std::path::PathBuf;

const EXIT_SUCCESS: i32 = 0;
const EXIT_FAILURE: i32 = 1;

pub fn main_rustyrts(mut callbacks: impl Callbacks + Send, fail_fast: bool) {
    rustc_log::init_logger(LoggerConfig::from_env("RUSTC")).unwrap();
    setup_logger();

    let result = rustc_driver::catch_fatal_errors(move || {
        let mut rustc_args = std::env::args()
            .map(|arg| {
                // when running blackbox tests, this ensures that stable crate ids do not change if features are enabled
                if std::env::var(ENV_BLACKBOX_TEST).is_ok() && arg.starts_with("metadata=") {
                    return "metadata=".to_string();
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

        let run_compiler = rustc_driver::RunCompiler::new(&rustc_args, &mut callbacks);
        run_compiler.run()
    });

    let result = result.unwrap();

    if fail_fast {
        std::process::exit(EXIT_FAILURE);
    }

    let exit_code = match result {
        Ok(()) => EXIT_SUCCESS,
        Err(_) => EXIT_FAILURE,
    };

    std::process::exit(exit_code);
}
