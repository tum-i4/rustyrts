use clap::Parser;
use std::{ffi::OsString, path::PathBuf, process::Command};

use rustc_build_sysroot::{BuildMode, SysrootBuilder, SysrootConfig};
use rustc_version::VersionMeta;

//######################################################################################################################
// Inspired from Miri and how it generates a cutom sysroot
//######################################################################################################################

#[derive(Parser, Debug)]
struct Args {
    output_dir: PathBuf,

    #[arg(long)]
    target: Option<String>,

    #[arg(long)]
    features: Option<Vec<String>>,
}

fn main() {
    let args = Args::parse();

    let rustc_version =
        VersionMeta::for_command(rustc()).expect("failed to determine underlying rustc version");

    let host = &rustc_version.host;
    let target = args.target.as_ref().unwrap_or(host);

    let std_features = args
        .features
        .unwrap_or(vec!["backtrace".to_string(), "panic_unwind".to_string()]);

    let sysroot_config = SysrootConfig::WithStd { std_features };
    let sysroot_dir = args.output_dir;

    let rustflags: &[&str] = &["-Zalways_encode_mir"];

    let cargo_cmd = cargo();

    let rust_src = rustc_build_sysroot::rustc_sysroot_src(rustc())
            .expect("Did not find sources in sysroot directory - please add rust-src (rustup component add rust-src)");

    SysrootBuilder::new(&sysroot_dir, target)
        .build_mode(BuildMode::Build)
        .rustc_version(rustc_version.clone())
        .sysroot_config(sysroot_config)
        .rustflags(rustflags)
        .cargo(cargo_cmd)
        .build_from_source(&rust_src)
        .unwrap_or_else(|err| panic!("failed to build sysroot: {err:?}"));

    println!("Installed custom sysroot at {}", sysroot_dir.display());
}

fn cargo() -> Command {
    Command::new(std::env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo")))
}

fn rustc() -> Command {
    Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| OsString::from("rustc")))
}
