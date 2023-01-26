#![feature(rustc_private)]
#![feature(box_patterns)]
#![feature(core_intrinsics)]
#![feature(box_syntax)]
#![allow(mutable_transmutes)]

extern crate rustc_ast;
extern crate rustc_data_structures;
extern crate rustc_driver;
extern crate rustc_error_codes;
extern crate rustc_errors;
extern crate rustc_hash;
extern crate rustc_hir;
extern crate rustc_incremental;
extern crate rustc_index;
extern crate rustc_infer;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_mir_build;
extern crate rustc_mir_transform;
extern crate rustc_query_impl;
extern crate rustc_query_system;
extern crate rustc_resolve;

extern crate rustc_session;
extern crate rustc_span;
extern crate rustc_target;

extern crate itertools;
extern crate log;
extern crate queues;

pub mod analysis {
    pub mod callback;
    pub mod checksums;
    pub mod util;
    pub mod visitor;
}

pub mod graph {
    pub mod graph;
}

pub mod paths;
pub mod utils;
