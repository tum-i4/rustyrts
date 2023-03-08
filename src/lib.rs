#![feature(rustc_private)]
#![allow(mutable_transmutes)]

// required for calling the compiler and providing callbacks
extern crate rustc_driver;
extern crate rustc_interface;

// required for hashing and computing checksums
extern crate rustc_data_structures;
extern crate rustc_hash;
extern crate rustc_session;

// required for analyzing and modifying the MIR
extern crate rustc_abi;
extern crate rustc_ast;
extern crate rustc_hir;
extern crate rustc_middle;

// required for running compiler on strings during testing
extern crate rustc_error_codes;
extern crate rustc_errors;
extern crate rustc_span;

pub mod static_rts {
    pub mod callback;
    pub mod graph;
    pub mod visitor;
}

pub mod dynamic_rts {
    pub mod callback;
    pub mod defid_util;
    pub mod mir_util;
    pub mod visitor;
}

pub mod callbacks_shared;
pub mod checksums;
pub mod constants;
pub mod fs_utils;
pub mod names;
pub mod utils;
