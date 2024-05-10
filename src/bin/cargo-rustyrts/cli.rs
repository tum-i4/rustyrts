use crate::commands;
use anyhow::Context;
use cargo::core::features::HIDDEN;
use cargo::core::shell::Shell;
use cargo::core::{features, CliUnstable};
use cargo::util::command_prelude::*;
use cargo::util::style;
use cargo::{self, drop_println, CargoResult, CliResult, Config};
use clap::{builder::UnknownArgumentValueParser, Arg, ArgMatches};
use std::ffi::OsString;

pub fn main(config: &mut LazyConfig) -> CliResult {
    let args = cli().try_get_matches()?.remove_subcommand().unwrap().1;

    // Update the process-level notion of cwd
    // This must be completed before config is initialized
    assert_eq!(config.is_init(), false);
    if let Some(new_cwd) = args.get_one::<std::path::PathBuf>("directory") {
        // This is a temporary hack. This cannot access `Config`, so this is a bit messy.
        // This does not properly parse `-Z` flags that appear after the subcommand.
        // The error message is not as helpful as the standard one.
        let nightly_features_allowed = matches!(&*features::channel(), "nightly" | "dev");
        if !nightly_features_allowed
            || (nightly_features_allowed
                && !args
                    .get_many("unstable-features")
                    .map(|mut z| z.any(|value: &String| value == "unstable-options"))
                    .unwrap_or(false))
        {
            return Err(anyhow::format_err!(
                "the `-C` flag is unstable, \
                 pass `-Z unstable-options` on the nightly channel to enable it"
            )
            .into());
        }
        std::env::set_current_dir(&new_cwd).context("could not change to requested directory")?;
    }

    // CAUTION: Be careful with using `config` until it is configured below.
    // In general, try to avoid loading config values unless necessary (like
    // the [alias] table).
    let config = config.get_mut();

    let expanded_args = args;

    if expanded_args
        .get_one::<String>("unstable-features")
        .map(String::as_str)
        == Some("help")
    {
        let options = CliUnstable::help();
        let non_hidden_options: Vec<(String, String)> = options
            .iter()
            .filter(|(_, help_message)| *help_message != HIDDEN)
            .map(|(name, help)| (name.to_string(), help.to_string()))
            .collect();
        let longest_option = non_hidden_options
            .iter()
            .map(|(option_name, _)| option_name.len())
            .max()
            .unwrap_or(0);
        let help_lines: Vec<String> = non_hidden_options
            .iter()
            .map(|(option_name, option_help_message)| {
                let option_name_kebab_case = option_name.replace("_", "-");
                let padding = " ".repeat(longest_option - option_name.len()); // safe to subtract
                format!(
                    "    -Z {}{} -- {}",
                    option_name_kebab_case, padding, option_help_message
                )
            })
            .collect();
        let joined = help_lines.join("\n");
        drop_println!(
            config,
            "
Available unstable (nightly-only) flags:

{}

Run with 'cargo -Z [FLAG] [COMMAND]'",
            joined
        );
        if !config.nightly_features_allowed {
            drop_println!(
                config,
                "\nUnstable flags are only available on the nightly channel \
                 of Cargo, but this is the `{}` channel.\n\
                 {}",
                features::channel(),
                features::SEE_CHANNELS
            );
        }
        drop_println!(
            config,
            "\nSee https://doc.rust-lang.org/nightly/cargo/reference/unstable.html \
             for more information about these flags."
        );
        return Ok(());
    }

    let is_verbose = expanded_args.verbose() > 0;
    if expanded_args.flag("version") {
        let mut cmd = std::process::Command::new(
            std::env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo")),
        );
        cmd.arg("--version");
        if is_verbose {
            cmd.arg("-v");
        }
        let _ = cmd.output();
        return Ok(());
    }

    let (cmd, subcommand_args) = match expanded_args.subcommand() {
        Some((cmd, args)) => (cmd, args),
        _ => {
            // No subcommand provided.
            cli().print_help()?;
            return Ok(());
        }
    };
    let exec = Exec::infer(cmd)?;
    config_configure(config, &expanded_args, subcommand_args)?;
    super::init_git(config);

    exec.exec(config, subcommand_args)
}

fn config_configure(
    config: &mut Config,
    args: &ArgMatches,
    subcommand_args: &ArgMatches,
) -> CliResult {
    let arg_target_dir = &subcommand_args.value_of_path("target-dir", config);
    let verbose = args.verbose();
    // quiet is unusual because it is redefined in some subcommands in order
    // to provide custom help text.
    let quiet = args.flag("quiet") || subcommand_args.flag("quiet");
    let color = args.get_one::<String>("color").map(String::as_str);
    let frozen = args.flag("frozen");
    let locked = args.flag("locked");
    let offline = args.flag("offline");
    let mut unstable_flags = Vec::new();
    if let Some(values) = args.get_many::<String>("unstable-features") {
        unstable_flags.extend(values.cloned());
    }
    let mut config_args = Vec::new();
    if let Some(values) = args.get_many::<String>("config") {
        config_args.extend(values.cloned());
    }
    config.configure(
        verbose,
        quiet,
        color,
        frozen,
        locked,
        offline,
        arg_target_dir,
        &unstable_flags,
        &config_args,
    )?;
    Ok(())
}

enum Exec {
    Builtin(commands::Exec),
}

impl Exec {
    fn infer(cmd: &str) -> CargoResult<Self> {
        if let Some(exec) = commands::command_exec(cmd) {
            Ok(Self::Builtin(exec))
        } else {
            panic!();
        }
    }

    fn exec(self, config: &mut Config, subcommand_args: &ArgMatches) -> CliResult {
        match self {
            Self::Builtin(exec) => exec(config, subcommand_args),
        }
    }
}

pub fn cli() -> Command {
    let styles = {
        clap::builder::styling::Styles::styled()
            .header(style::HEADER)
            .usage(style::USAGE)
            .literal(style::LITERAL)
            .placeholder(style::PLACEHOLDER)
            .error(style::ERROR)
            .valid(style::VALID)
            .invalid(style::INVALID)
    };

    Command::new("cargo-rustyrts").bin_name("cargo").subcommand(
        subcommand("rustyrts")
            .styles(styles)
            .arg(flag("version", "Print version info and exit").short('V'))
            .arg(
                opt(
                    "verbose",
                    "Use verbose output (-vv very verbose/build.rs output)",
                )
                .short('v')
                .action(ArgAction::Count),
            )
            .arg(flag("quiet", "Do not print cargo log messages").short('q'))
            .arg(opt("color", "Coloring: auto, always, never").value_name("WHEN"))
            .arg(
                Arg::new("directory")
                    .help("Change to DIRECTORY before doing anything (nightly-only)")
                    .short('C')
                    .value_name("DIRECTORY")
                    .value_hint(clap::ValueHint::DirPath)
                    .value_parser(clap::builder::ValueParser::path_buf()),
            )
            .arg(
                flag("frozen", "Require Cargo.lock and cache are up to date")
                    .help_heading(heading::MANIFEST_OPTIONS),
            )
            .arg(
                flag("locked", "Require Cargo.lock is up to date")
                    .help_heading(heading::MANIFEST_OPTIONS),
            )
            .arg(
                flag("offline", "Run without accessing the network")
                    .help_heading(heading::MANIFEST_OPTIONS),
            )
            // Better suggestion for the unsupported short config flag.
            .arg(
                Arg::new("unsupported-short-config-flag")
                    .help("")
                    .short('c')
                    .value_parser(UnknownArgumentValueParser::suggest_arg("--config"))
                    .action(ArgAction::SetTrue)
                    .hide(true),
            )
            .arg(multi_opt(
                "config",
                "KEY=VALUE",
                "Override a configuration value",
            ))
            // Better suggestion for the unsupported lowercase unstable feature flag.
            .arg(
                Arg::new("unsupported-lowercase-unstable-feature-flag")
                    .help("")
                    .short('z')
                    .value_parser(UnknownArgumentValueParser::suggest_arg("-Z"))
                    .action(ArgAction::SetTrue)
                    .hide(true),
            )
            .arg(
                Arg::new("unstable-features")
                    .help("Unstable (nightly-only) flags to Cargo, see 'cargo -Z help' for details")
                    .short('Z')
                    .value_name("FLAG")
                    .action(ArgAction::Append),
            )
            .subcommands(commands::commands()),
    )
}

/// Delay loading [`Config`] until access.
///
/// In the common path, the [`Config`] is dependent on CLI parsing and shouldn't be loaded until
/// after that is done but some other paths (like fix or earlier errors) might need access to it,
/// so this provides a way to share the instance and the implementation across these different
/// accesses.
pub struct LazyConfig {
    config: Option<Config>,
}

impl LazyConfig {
    pub fn new() -> Self {
        Self { config: None }
    }

    /// Check whether the config is loaded
    ///
    /// This is useful for asserts in case the environment needs to be setup before loading
    pub fn is_init(&self) -> bool {
        self.config.is_some()
    }

    /// Get the config, loading it if needed
    ///
    /// On error, the process is terminated
    pub fn get(&mut self) -> &Config {
        self.get_mut()
    }

    /// Get the config, loading it if needed
    ///
    /// On error, the process is terminated
    pub fn get_mut(&mut self) -> &mut Config {
        self.config.get_or_insert_with(|| match Config::default() {
            Ok(cfg) => cfg,
            Err(e) => {
                let mut shell = Shell::new();
                cargo::exit_with_error(e.into(), &mut shell)
            }
        })
    }
}

#[test]
fn verify_cli() {
    cli().debug_assert();
}
