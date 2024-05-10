#![feature(rustc_private)]
#![allow(mutable_transmutes)]

// required for resolving names
extern crate rustc_resolve;

// required for calling the compiler and providing callbacks
extern crate rustc_driver;
extern crate rustc_interface;

// required for hashing and computing checksums
extern crate rustc_data_structures;
extern crate rustc_hash;
extern crate rustc_query_system;
extern crate rustc_session;

// required for analyzing and modifying the MIR
extern crate rustc_abi;
extern crate rustc_ast;
extern crate rustc_attr;
extern crate rustc_const_eval;
extern crate rustc_feature;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;
extern crate rustc_type_ir;

extern crate cargo;

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
