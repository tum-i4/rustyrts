#![allow(dead_code)]

pub const VERBOSE_COUNT: usize = 16;

//######################################################################################################################
// Environment variables

/// Either "static" or "dynamic"
pub const ENV_RUSTYRTS_MODE: &str = "RUSTYRTS_MODE";

/// Can be set to overwrite the executable that is used when rustc is invoked
pub const ENV_RUSTC_WRAPPER: &str = "RUSTC_WRAPPER";

/// Can be set to control the behvior of rustdoc
pub const ENV_RUSTDOCFLAGS: &str = "RUSTDOCFLAGS";

/// Is set by cargo
pub const ENV_TARGET_DIR: &str = "CARGO_TARGET_DIR";

/// Used to buffer arguments to rustc
pub const ENV_RUSTYRTS_ARGS: &str = "RUSTYRTS_ARGS";

/// Used to specify whether rustyrts should provide verbose output
pub const ENV_RUSTYRTS_VERBOSE: &str = "RUSTYRTS_VERBOSE";

/// Used to specify the log level
pub const ENV_RUSTYRTS_LOG: &str = "RUSTYRTS_LOG";

/// May be used to skip the analysis
pub const ENV_SKIP_ANALYSIS: &str = "RUSTYRTS_SKIP";

pub const ENV_BLACKBOX_TEST: &str = "RUSTYRTS_BLACKBOX_TEST";

/// Indicates whether the crate that is currently compiled is doctested
pub const ENV_DOCTESTED: &str = "RUSTYRTS_DOCTESTED";

//######################################################################################################################
// File endings or names

pub const DIR_STATIC: &str = ".rts_static";
pub const DIR_DYNAMIC: &str = ".rts_dynamic";
pub const DIR_DOCTEST: &str = ".rts_doctest";

pub const FILE_COMPLETE_GRAPH: &str = "!complete_graph.dot";

pub const ENDING_TRACE: &str = "trace";
pub const ENDING_CHANGES: &str = "changes"; // TODO: actively use extension in pathbuf
pub const ENDING_CHECKSUM: &str = "checksum";
pub const ENDING_CHECKSUM_OLD: &str = "checksum_old";
pub const ENDING_CHECKSUM_VTBL: &str = "checksum_vtbl";
pub const ENDING_CHECKSUM_VTBL_OLD: &str = "checksum_vtbl_old";
pub const ENDING_CHECKSUM_CONST: &str = "checksum_const";
pub const ENDING_CHECKSUM_CONST_OLD: &str = "checksum_const_old";
pub const ENDING_TEST: &str = "test";
pub const ENDING_GRAPH: &str = "dot";

#[cfg(unix)]
pub const ENDING_PROCESS_TRACE: &str = ".process_trace";

//######################################################################################################################
// Edge cases that need special treatment

pub const SUFFIX_DYN: &str = "!dyn";

pub const EDGE_CASES_NO_TRACE: &[&str] = &[
    "__rg_alloc",
    "__rg_dealloc",
    "__rg_realloc",
    "__rg_alloc_zeroed",
    "as GlobalAlloc>::alloc",
    "as GlobalAlloc>::dealloc",
    "as GlobalAlloc>::realloc",
    "as GlobalAlloc>::alloc_zeroed",
];
