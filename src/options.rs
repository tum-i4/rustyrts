// Copyright 2021-2024 Martin Pool

//! Global in-process options for experimenting on mutants.
//!
//! The [Options] structure is built from command-line options and then widely passed around.
//! Options are also merged from the [config] after reading the command line arguments.

use std::time::Duration;

use anyhow::Context;
use camino::Utf8PathBuf;
use clap::ValueEnum;
use globset::{Glob, GlobSet, GlobSetBuilder};
use regex::RegexSet;
use serde::Deserialize;
use strum::{Display, EnumString};
use tracing::warn;

use crate::config::Config;
use crate::*;

/// Options for mutation testing, based on both command-line arguments and the
/// config file.
#[derive(Default, Debug, Clone)]
pub struct Options {
    /// Run tests in an unmutated tree?
    pub baseline: BaselineStrategy,

    /// Don't run the tests, just see if each mutant builds.
    pub check_only: bool,

    /// Don't copy files matching gitignore patterns to build directories.
    pub gitignore: bool,

    /// Don't copy at all; run tests in the source directory.
    pub in_place: bool,

    /// Don't delete scratch directories.
    pub leak_dirs: bool,

    /// The time limit for test tasks, if set.
    ///
    /// If this is not set by the user it's None, in which case there is no time limit
    /// on the baseline test, and then the mutated tests get a multiple of the time
    /// taken by the baseline test.
    pub test_timeout: Option<Duration>,

    /// The minimum test timeout, as a floor on the autoset value.
    pub minimum_test_timeout: Duration,

    pub print_caught: bool,
    pub print_unviable: bool,

    pub show_times: bool,

    /// Show logs even from mutants that were caught, or source/unmutated builds.
    pub show_all_logs: bool,

    /// List mutants with line and column numbers.
    pub show_line_col: bool,

    /// Test mutants in random order.
    ///
    /// This is now the default, so that repeated partial runs are more likely to find
    /// interesting results.
    pub shuffle: bool,

    /// Additional arguments for every cargo invocation.
    pub additional_cargo_args: Vec<String>,

    /// Additional arguments to `cargo test`.
    pub additional_cargo_test_args: Vec<String>,

    /// Files to examine.
    pub examine_globset: Option<GlobSet>,

    /// Files to exclude.
    pub exclude_globset: Option<GlobSet>,

    /// Mutants to examine, as a regexp matched against the full name.
    pub examine_names: RegexSet,

    /// Mutants to skip, as a regexp matched against the full name.
    pub exclude_names: RegexSet,

    /// Create `mutants.out` within this directory (by default, the source directory).
    pub output_in_dir: Option<Utf8PathBuf>,

    /// Run this many `cargo build` or `cargo test` tasks in parallel.
    pub jobs: Option<usize>,

    pub emit_mir: bool,

    /// Insert these values as errors from functions returning `Result`.
    pub error_values: Vec<String>,

    /// Show ANSI colors.
    pub colors: Colors,

    /// List mutants in json, etc.
    pub emit_json: bool,

    /// Emit diffs showing just what changed.
    pub emit_diffs: bool,

    /// The tool to use to run tests.
    pub test_tool: TestTool,
}

/// Choice of tool to use to run tests.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, EnumString, Display, Deserialize)]
#[strum(serialize_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum TestTool {
    /// Use `cargo test`, the default.
    #[default]
    Cargo,

    /// Use `cargo nextest`.
    Nextest,

    // Use 'cargo rustyrts basic'
    Basic,

    // Use 'cargo rustyrts dynamic'
    Dynamic,

    // Use 'cargo rustyrts static'
    Static,
}

/// Join two slices into a new vector.
fn join_slices(a: &[String], b: &[String]) -> Vec<String> {
    let mut v = Vec::with_capacity(a.len() + b.len());
    v.extend_from_slice(a);
    v.extend_from_slice(b);
    v
}

/// Should ANSI colors be drawn?
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Display, Deserialize, ValueEnum)]
#[strum(serialize_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum Colors {
    #[default]
    Auto,
    Always,
    Never,
}

impl Colors {
    /// If colors were forced on or off by the user through an option or
    /// environment variable, return that value.
    ///
    /// Otherwise, return None, meaning we should decide based on the
    /// detected terminal characteristics.
    pub fn forced_value(&self) -> Option<bool> {
        // From https://bixense.com/clicolors/
        if env::var("NO_COLOR").map_or(false, |x| x != "0") {
            Some(false)
        } else if env::var("CLICOLOR_FORCE").map_or(false, |x| x != "0") {
            Some(true)
        } else {
            match self {
                Colors::Always => Some(true),
                Colors::Never => Some(false),
                Colors::Auto => None, // library should decide
            }
        }
    }

    pub fn active_stdout(&self) -> bool {
        self.forced_value()
            .unwrap_or_else(::console::colors_enabled)
    }
}

impl Options {
    /// Build options by merging command-line args and config file.
    pub(crate) fn new(args: &Args, config: &Config) -> Result<Options> {
        if args.no_copy_target {
            warn!("--no-copy-target is deprecated and has no effect; target/ is never copied");
        }

        let minimum_test_timeout = Duration::from_secs_f64(
            args.minimum_test_timeout
                .or(config.minimum_test_timeout)
                .unwrap_or(20f64),
        );

        let json_args = if args.json {
            let mut vec = Vec::new();
            if !args.cargo_test_args.iter().any(|s| s == "--") {
                vec.push("--");
            }
            vec.extend_from_slice(&["-Zunstable-options", "--format=json", "--report-time"]);
            vec
        } else {
            Vec::new()
        };

        let gitignore = {
            match args.test_tool {
                Some(TestTool::Dynamic | TestTool::Static) => false,
                _ => args.gitignore,
            }
        };

        let options = Options {
            additional_cargo_args: join_slices(&args.cargo_arg, &config.additional_cargo_args),
            additional_cargo_test_args: args
                .cargo_test_args
                .iter()
                .cloned()
                .chain(config.additional_cargo_test_args.iter().cloned())
                .chain(json_args.into_iter().map(|s| s.to_string()))
                .collect(),
            baseline: args.baseline,
            check_only: args.check,
            colors: args.colors,
            emit_json: args.json,
            emit_diffs: args.diff,
            error_values: join_slices(&args.error, &config.error_values),
            examine_names: RegexSet::new(or_slices(&args.examine_re, &config.examine_re))
                .context("Failed to compile examine_re regex")?,
            exclude_names: RegexSet::new(or_slices(&args.exclude_re, &config.exclude_re))
                .context("Failed to compile exclude_re regex")?,
            examine_globset: build_glob_set(or_slices(&args.file, &config.examine_globs))?,
            exclude_globset: build_glob_set(or_slices(&args.exclude, &config.exclude_globs))?,
            gitignore,
            in_place: args.in_place,
            jobs: args.jobs,
            leak_dirs: args.leak_dirs,
            minimum_test_timeout,
            output_in_dir: args.output.clone(),
            print_caught: args.caught,
            print_unviable: args.unviable,
            shuffle: args.shuffle,
            show_line_col: args.line_col,
            show_times: !args.no_times,
            show_all_logs: args.all_logs,
            test_timeout: args.timeout.map(Duration::from_secs_f64),
            test_tool: args.test_tool.or(config.test_tool).unwrap_or_default(),
            emit_mir: args.emit_mir,
        };
        options.error_values.iter().for_each(|e| {
            if e.starts_with("Err(") {
                warn!(
                    "error_value option gives the value of the error, and probably should not start with Err(: got {}",
                    e
                );
            }
        });
        Ok(options)
    }
}

/// If the first slices is non-empty, return that, otherwise the second.
fn or_slices<'a: 'c, 'b: 'c, 'c, T>(a: &'a [T], b: &'b [T]) -> &'c [T] {
    if a.is_empty() {
        b
    } else {
        a
    }
}

fn build_glob_set<S: AsRef<str>, I: IntoIterator<Item = S>>(
    glob_set: I,
) -> Result<Option<GlobSet>> {
    let mut glob_set = glob_set.into_iter().peekable();
    if glob_set.peek().is_none() {
        return Ok(None);
    }

    let mut builder = GlobSetBuilder::new();
    for glob_str in glob_set {
        let glob_str = glob_str.as_ref();
        if glob_str.contains('/') || glob_str.contains(std::path::MAIN_SEPARATOR) {
            builder.add(Glob::new(glob_str)?);
        } else {
            builder.add(Glob::new(&format!("**/{glob_str}"))?);
        }
    }
    Ok(Some(builder.build()?))
}

#[cfg(test)]
mod test {
    use std::io::Write;

    use indoc::indoc;
    use tempfile::NamedTempFile;

    use super::*;

    #[test]
    fn default_options() {
        let args = Args::parse_from(["mutants-rts"]);
        let options = Options::new(&args, &Config::default()).unwrap();
        assert!(!options.check_only);
        assert_eq!(options.test_tool, TestTool::Cargo);
    }

    // Nexteest is not supported
    // #[test]
    // fn options_from_test_tool_arg() {
    // let args = Args::parse_from(["mutants-rts", "--test-tool", "nextest"]);
    // let options = Options::new(&args, &Config::default()).unwrap();
    // assert_eq!(options.test_tool, TestTool::Nextest);
    // }

    #[test]
    fn options_from_baseline_arg() {
        let args = Args::parse_from(["mutants-rts", "--baseline", "skip"]);
        let options = Options::new(&args, &Config::default()).unwrap();
        assert_eq!(options.baseline, BaselineStrategy::Skip);

        let args = Args::parse_from(["mutants-rts", "--baseline", "run"]);
        let options = Options::new(&args, &Config::default()).unwrap();
        assert_eq!(options.baseline, BaselineStrategy::Run);

        let args = Args::parse_from(["mutants-rts"]);
        let options = Options::new(&args, &Config::default()).unwrap();
        assert_eq!(options.baseline, BaselineStrategy::Run);
    }

    #[test]
    fn test_tool_from_config() {
        let config = indoc! { r#"
            test_tool = "nextest"
        "#};
        let mut config_file = NamedTempFile::new().unwrap();
        config_file.write_all(config.as_bytes()).unwrap();
        let args = Args::parse_from(["mutants-rts"]);
        let config = Config::read_file(config_file.path()).unwrap();
        let options = Options::new(&args, &config).unwrap();
        assert_eq!(options.test_tool, TestTool::Nextest);
    }
}
