#![feature(rustc_private)]
#![feature(let_chains)]
#![allow(mutable_transmutes)]

// required for calling the compiler and providing callbacks
extern crate rustc_driver;
extern crate rustc_interface;
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

// required by code from librustdoc
extern crate rustc_resolve;

// TODO: maybe this could be moved to the commands module?
extern crate cargo;

pub mod doctest_rts;
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
