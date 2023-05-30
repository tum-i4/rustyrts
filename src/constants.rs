#![allow(dead_code)]

pub const DESC_FLAG: &str = "--descriptions";

//######################################################################################################################
// Environment variables

/// Either "static" or "dynamic"
pub const ENV_RUSTYRTS_MODE: &str = "RUSTYRTS_MODE";

/// Can be set to overwrite the executable that is used when rustc is invoked
pub const ENV_RUSTC_WRAPPER: &str = "RUSTC_WRAPPER";

/// Is set by cargo
pub const ENV_TARGET_DIR: &str = "CARGO_TARGET_DIR";

/// Used to transmit the directory of the current project
/// that is not obvious when compiling dependencies
pub const ENV_PROJECT_DIR: &str = "PROJECT_DIR";

/// Used to buffer arguments to rustc
pub const ENV_RUSTYRTS_ARGS: &str = "rustyrts_args";

/// Used to specify whether rustyrts should provide verbose output
pub const ENV_RUSTYRTS_VERBOSE: &str = "RUSTYRTS_VERBOSE";

//######################################################################################################################
// File endings or names

pub const DIR_STATIC: &str = ".rts_static";
pub const DIR_DYNAMIC: &str = ".rts_dynamic";

pub const FILE_COMPLETE_GRAPH: &str = "!complete_graph.dot";

pub const ENDING_TRACE: &str = ".trace";
pub const ENDING_CHANGES: &str = ".changes";
pub const ENDING_CHECKSUM: &str = ".checksum";
pub const ENDING_CHECKSUM_VTBL: &str = ".checksum_vtbl";
pub const ENDING_TEST: &str = ".test";
pub const ENDING_GRAPH: &str = ".dot";

#[cfg(feature = "ctfe")]
pub const ENDING_CHECKSUM_CTFE: &str = ".checksum_ctfe";

#[cfg(unix)]
pub const ENDING_PROCESS_TRACE: &str = ".process_trace";

//######################################################################################################################
// Edge cases that need special treatment

pub const EDGE_CASE_FROM_RESIDUAL: &str = "ops::try_trait::FromResidual::from_residual";

pub const EDGE_CASES_NO_TRACE: &[&str] = &[
    "_::__rg_alloc",
    "_::__rg_dealloc",
    "_::__rg_realloc",
    "_::__rg_alloc_zeroed",
    "as core::alloc::global::GlobalAlloc>::alloc",
    "as core::alloc::global::GlobalAlloc>::dealloc",
    "as core::alloc::global::GlobalAlloc>::realloc",
    "as core::alloc::global::GlobalAlloc>::alloc_zeroed",
];
