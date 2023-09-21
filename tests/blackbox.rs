use lazy_static::lazy_static;
use std::{fs::create_dir_all, path::PathBuf};
use std::{path::Path, process::Command};
use test_case::test_case;

use rustyrts::constants::{
    ENV_BLACKBOX_TEST, ENV_SKIP_ANALYSIS, ENV_TARGET_DIR, ENV_TARGET_DIR_OVERRIDE,
};
use tempdir::TempDir;

enum Mode {
    Dynamic,
    Static,
}

impl Into<&str> for &Mode {
    fn into(self) -> &'static str {
        match self {
            Mode::Dynamic => "dynamic",
            Mode::Static => "static",
        }
    }
}

fn command(mode: &Mode, dir: &PathBuf, target_dir: &Path, feature: Option<&str>) -> Command {
    let mut ret = Command::new(env!("CARGO_BIN_EXE_cargo-rustyrts"));
    ret.arg("rustyrts").arg(Into::<&str>::into(mode));
    ret.current_dir(dir);

    if let Some(name) = feature {
        ret.arg("--")
            .arg("--features")
            .arg(name)
            .arg("--")
            .arg("--")
            .arg("--features")
            .arg(name);
    }

    ret.env(ENV_TARGET_DIR, target_dir)
        .env(ENV_BLACKBOX_TEST, "true")
        .env(
            ENV_TARGET_DIR_OVERRIDE,
            std::env::var(ENV_TARGET_DIR).unwrap(),
        );

    ret.env_remove(ENV_SKIP_ANALYSIS);

    ret
}

lazy_static! {
    static ref PATH: PathBuf = {
        let mut path_buf = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path_buf.push("test-data");
        path_buf.push("blackbox");
        path_buf
    };
}

#[test_case(Mode::Dynamic; "dynamic_check_same_crate_id")]
#[test_case(Mode::Static; "static_check_same_crate_id")]
fn check_same_crate_id(mode: Mode) {
    let mut dir = PATH.clone();
    dir.push("check_same_crate_id");

    let target_dir = TempDir::new_in(
        env!("CARGO_TARGET_TMPDIR"),
        dir.file_name().unwrap().to_str().unwrap(),
    )
    .unwrap();

    {
        println!("-------- baseline --------");
        let result = command(&mode, &dir, target_dir.path(), None)
            .output()
            .unwrap();
        println!("Stdout: {}", String::from_utf8(result.stdout).unwrap());
        println!("Stderr: {}", String::from_utf8(result.stderr).unwrap());
        assert!(!result.status.success());
    }

    {
        println!("-------- with changes --------");
        let result = command(&mode, &dir, target_dir.path(), None)
            .output()
            .unwrap();
        println!("Stdout: {}", String::from_utf8(result.stdout).unwrap());
        println!("Stderr: {}", String::from_utf8(result.stderr).unwrap());
        assert!(result.status.success());
    }
}

#[test_case(Mode::Dynamic, "adt", "changes_display")]
#[test_case(Mode::Static, "adt", "changes_display")]
#[test_case(Mode::Dynamic, "adt", "changes_debug")]
#[test_case(Mode::Static, "adt", "changes_debug")]
#[test_case(Mode::Dynamic, "adt", "changes_drop")]
#[test_case(Mode::Static, "adt", "changes_drop")]
#[test_case(Mode::Dynamic, "command", "changes_return")]
#[test_case(Mode::Dynamic, "dynamic", "changes_direct")]
#[test_case(Mode::Static, "dynamic", "changes_direct")]
#[test_case(Mode::Dynamic, "dynamic", "changes_indirect")]
#[test_case(Mode::Static, "dynamic", "changes_indirect")]
#[test_case(Mode::Dynamic, "dynamic", "changes_generic")]
#[test_case(Mode::Static, "dynamic", "changes_generic")]
#[test_case(Mode::Dynamic, "dynamic", "changes_static")]
#[test_case(Mode::Static, "dynamic", "changes_static")]
#[test_case(Mode::Dynamic, "assoc_items", "changes_string")]
#[test_case(Mode::Static, "assoc_items", "changes_string")]
#[test_case(Mode::Dynamic, "assoc_items", "changes_assoc_const")]
#[test_case(Mode::Static, "assoc_items", "changes_assoc_const")]
// #[test_case(Mode::Dynamic, "assoc_items", "changes_assoc_type")] // Does not work yet
// #[test_case(Mode::Static, "assoc_items", "changes_assoc_type")]
#[test_case(Mode::Dynamic, "lazy", "changes_lazy")]
#[test_case(Mode::Static, "lazy", "changes_lazy")]
#[test_case(Mode::Dynamic, "static_var", "changes_immutable")]
#[test_case(Mode::Static, "static_var", "changes_mutable")]
fn blackbox_test_affected(mode: Mode, name: &str, feature: &str) {
    let mut dir = PATH.clone();
    dir.push(name);

    let target_dir = TempDir::new_in(
        env!("CARGO_TARGET_TMPDIR"),
        dir.file_name().unwrap().to_str().unwrap(),
    )
    .unwrap();

    {
        println!("-------- baseline --------");
        let result = command(&mode, &dir, target_dir.path(), None)
            .output()
            .unwrap();
        println!("Stdout: {}", String::from_utf8(result.stdout).unwrap());
        println!("Stderr: {}", String::from_utf8(result.stderr).unwrap());
        assert!(result.status.success());
    }

    {
        println!("-------- with changes --------");
        let result = command(&mode, &dir, target_dir.path(), Some(feature))
            .output()
            .unwrap();
        println!("Stdout: {}", String::from_utf8(result.stdout).unwrap());
        println!("Stderr: {}", String::from_utf8(result.stderr).unwrap());
        assert!(!result.status.success());
    }
}

#[test_case(
    Mode::Dynamic,
    "threading",
    "test1_panic",
    "test1_panic, changes_test2"
)]
#[test_case(Mode::Static, "threading", "test2_panic", "test2_panic, changes_test1")]
fn blackbox_test_not_affected(
    mode: Mode,
    name: &str,
    features_baseline: &str,
    features_changes: &str,
) {
    let mut dir = PATH.clone();
    dir.push(name);

    let target_dir = TempDir::new_in(
        env!("CARGO_TARGET_TMPDIR"),
        dir.file_name().unwrap().to_str().unwrap(),
    )
    .unwrap();
    create_dir_all(target_dir.path()).unwrap();

    {
        println!("-------- baseline --------");
        let result = command(&mode, &dir, target_dir.path(), Some(features_baseline))
            .output()
            .unwrap();
        println!("Stdout: {}", String::from_utf8(result.stdout).unwrap());
        println!("Stderr: {}", String::from_utf8(result.stderr).unwrap());
        assert!(!result.status.success());
    }

    {
        println!("-------- with changes --------");
        let result = command(&mode, &dir, target_dir.path(), Some(features_changes))
            .output()
            .unwrap();
        println!("Stdout: {}", String::from_utf8(result.stdout).unwrap());
        println!("Stderr: {}", String::from_utf8(result.stderr).unwrap());
        assert!(result.status.success());
    }
}
