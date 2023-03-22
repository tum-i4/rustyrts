#![allow(dead_code)]

//######################################################################################################################
// Environment variables

/// Either "static" or "dynamic"
pub const ENV_RUSTYRTS_MODE: &str = "RUSTYRTS_MODE";

/// Can be set to overwrite the executable that is used when rustc is invoked
pub const ENV_RUSTC_WRAPPER: &str = "RUSTC_WRAPPER";

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

pub const FILE_AFFECTED: &str = "affected";
pub const FILE_COMPLETE_GRAPH: &str = "!complete_graph.dot";

pub const ENDING_TRACE: &str = ".trace";
pub const ENDING_CHANGES: &str = ".changes";
pub const ENDING_CHECKSUM: &str = ".checksum";
pub const ENDING_TEST: &str = ".test";
pub const ENDING_GRAPH: &str = ".dot";
pub const ENDING_REEXPORTS: &str = ".exp";

#[cfg(target_family = "unix")]
pub const ENDING_PROCESS_TRACE: &str = ".process_trace";

pub const EDGE_CASE_FROM_RESIDUAL: &str = "core::ops::FromResidual::from_residual";
