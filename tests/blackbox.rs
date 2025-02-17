use lazy_static::lazy_static;
use std::{fs::create_dir_all, path::PathBuf};
use std::{path::Path, process::Command};
use test_case::test_case;

use rustyrts::constants::{ENV_BLACKBOX_TEST, ENV_TARGET_DIR};
use tempdir::TempDir;

enum Mode {
    Basic,
    Dynamic,
    Static,
}

impl From<&Mode> for &str {
    fn from(val: &Mode) -> Self {
        match val {
            Mode::Basic => "basic",
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
        ret.arg("--features").arg(name);
    }

    ret.arg("-v");

    ret.env(ENV_TARGET_DIR, target_dir);
    ret.env(ENV_BLACKBOX_TEST, "true");

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

#[test_case(Mode::Basic, "adt", "", "changes_display")]
#[test_case(Mode::Dynamic, "adt", "", "changes_display")]
#[test_case(Mode::Static, "adt", "", "changes_display")]
#[test_case(Mode::Basic, "adt", "", "changes_debug")]
#[test_case(Mode::Dynamic, "adt", "", "changes_debug")]
#[test_case(Mode::Static, "adt", "", "changes_debug")]
#[test_case(Mode::Basic, "drop", "drop_inner", "drop_inner,changes_drop")]
#[test_case(Mode::Dynamic, "drop", "drop_inner", "drop_inner,changes_drop")]
#[test_case(Mode::Static, "drop", "drop_inner", "drop_inner,changes_drop")]
#[test_case(Mode::Basic, "drop", "drop_outer", "drop_outer,changes_drop")]
#[test_case(Mode::Dynamic, "drop", "drop_outer", "drop_outer,changes_drop")]
#[test_case(Mode::Static, "drop", "drop_outer", "drop_outer,changes_drop")]
#[test_case(
    Mode::Basic,
    "drop",
    "drop_inner,drop_direct",
    "drop_inner,drop_direct,changes_drop"
)]
#[test_case(
    Mode::Dynamic,
    "drop",
    "drop_inner,drop_direct",
    "drop_inner,drop_direct,changes_drop"
)]
#[test_case(
    Mode::Static,
    "drop",
    "drop_inner,drop_direct",
    "drop_inner,drop_direct,changes_drop"
)]
#[test_case(
    Mode::Basic,
    "drop",
    "drop_outer,drop_direct",
    "drop_outer,drop_direct,changes_drop"
)]
#[test_case(
    Mode::Dynamic,
    "drop",
    "drop_outer,drop_direct",
    "drop_outer,drop_direct,changes_drop"
)]
#[test_case(
    Mode::Static,
    "drop",
    "drop_outer,drop_direct",
    "drop_outer,drop_direct,changes_drop"
)]
#[test_case(
    Mode::Basic,
    "drop",
    "drop_inner,drop_delegate",
    "drop_inner,drop_delegate,changes_drop"
)]
#[test_case(
    Mode::Dynamic,
    "drop",
    "drop_inner,drop_delegate",
    "drop_inner,drop_delegate,changes_drop"
)]
#[test_case(
    Mode::Static,
    "drop",
    "drop_inner,drop_delegate",
    "drop_inner,drop_delegate,changes_drop"
)]
#[test_case(
    Mode::Basic,
    "drop",
    "drop_outer,drop_delegate",
    "drop_outer,drop_delegate,changes_drop"
)]
#[test_case(
    Mode::Dynamic,
    "drop",
    "drop_outer,drop_delegate",
    "drop_outer,drop_delegate,changes_drop"
)]
#[test_case(
    Mode::Static,
    "drop",
    "drop_outer,drop_delegate",
    "drop_outer,drop_delegate,changes_drop"
)]
#[test_case(
    Mode::Basic,
    "drop",
    "drop_inner,drop_closure",
    "drop_inner,drop_closure,changes_drop"
)]
#[test_case(
    Mode::Dynamic,
    "drop",
    "drop_inner,drop_closure",
    "drop_inner,drop_closure,changes_drop"
)]
#[test_case(
    Mode::Static,
    "drop",
    "drop_inner,drop_closure",
    "drop_inner,drop_closure,changes_drop"
)]
#[test_case(
    Mode::Basic,
    "drop",
    "drop_outer,drop_closure",
    "drop_outer,drop_closure,changes_drop"
)]
#[test_case(
    Mode::Dynamic,
    "drop",
    "drop_outer,drop_closure",
    "drop_outer,drop_closure,changes_drop"
)]
#[test_case(
    Mode::Static,
    "drop",
    "drop_outer,drop_closure",
    "drop_outer,drop_closure,changes_drop"
)]
#[test_case(Mode::Basic, "command", "", "changes_return")]
#[test_case(Mode::Dynamic, "command", "", "changes_return")]
// #[test_case(Mode::Static, "command", "", "changes_return")] // Known source of unsafe behavior (Cross-process tets)
#[test_case(Mode::Basic, "dynamic", "", "changes_direct")]
#[test_case(Mode::Dynamic, "dynamic", "", "changes_direct")]
#[test_case(Mode::Static, "dynamic", "", "changes_direct")]
#[test_case(Mode::Basic, "dynamic", "", "changes_indirect")]
#[test_case(Mode::Dynamic, "dynamic", "", "changes_indirect")]
#[test_case(Mode::Static, "dynamic", "", "changes_indirect")]
#[test_case(Mode::Basic, "dynamic", "", "changes_generic")]
#[test_case(Mode::Dynamic, "dynamic", "", "changes_generic")]
#[test_case(Mode::Static, "dynamic", "", "changes_generic")]
#[test_case(Mode::Basic, "dynamic", "", "changes_static")]
#[test_case(Mode::Dynamic, "dynamic", "", "changes_static")]
#[test_case(Mode::Static, "dynamic", "", "changes_static")]
#[test_case(Mode::Basic, "dynamic", "", "changes_removed")]
#[test_case(Mode::Dynamic, "dynamic", "", "changes_removed")]
#[test_case(Mode::Static, "dynamic", "", "changes_removed")]
#[test_case(Mode::Basic, "assoc_items", "", "changes_string")]
#[test_case(Mode::Dynamic, "assoc_items", "", "changes_string")]
#[test_case(Mode::Static, "assoc_items", "", "changes_string")]
#[test_case(Mode::Basic, "assoc_items", "", "changes_assoc_const")]
#[test_case(Mode::Static, "assoc_items", "", "changes_assoc_const")]
#[test_case(Mode::Basic, "assoc_items", "", "changes_assoc_type")]
// #[test_case(Mode::Dynamic, "assoc_items", "", "changes_assoc_type")] // Known source of unsafe behavior (although rather technical)
// #[test_case(Mode::Static, "assoc_items", "", "changes_assoc_type")] // (Intrinsic, no change in MIR)
#[test_case(Mode::Basic, "lazy", "", "changes_lazy")]
#[test_case(Mode::Dynamic, "lazy", "", "changes_lazy")]
#[test_case(Mode::Static, "lazy", "", "changes_lazy")]
#[test_case(Mode::Basic, "static_var", "", "changes_immutable")]
#[test_case(Mode::Dynamic, "static_var", "", "changes_immutable")]
#[test_case(Mode::Static, "static_var", "", "changes_mutable")]
#[test_case(Mode::Basic, "fn_ptr", "test_direct", "test_direct,changes_fn")]
#[test_case(Mode::Dynamic, "fn_ptr", "test_direct", "test_direct,changes_fn")]
#[test_case(Mode::Static, "fn_ptr", "test_direct", "test_direct,changes_fn")]
#[test_case(Mode::Basic, "fn_ptr", "test_direct", "test_direct,changes_static")]
#[test_case(Mode::Dynamic, "fn_ptr", "test_direct", "test_direct,changes_static")]
#[test_case(Mode::Static, "fn_ptr", "test_direct", "test_direct,changes_static")]
#[test_case(Mode::Basic, "fn_ptr", "test_indirect", "test_indirect,changes_fn")]
#[test_case(Mode::Dynamic, "fn_ptr", "test_indirect", "test_indirect,changes_fn")]
#[test_case(Mode::Static, "fn_ptr", "test_indirect", "test_indirect,changes_fn")]
#[test_case(Mode::Basic, "derive", "", "changes_debug")]
#[test_case(Mode::Dynamic, "derive", "", "changes_debug")]
#[test_case(Mode::Static, "derive", "", "changes_debug")]
#[test_case(Mode::Basic, "derive", "", "changes_hash")]
#[test_case(Mode::Dynamic, "derive", "", "changes_hash")]
#[test_case(Mode::Static, "derive", "", "changes_hash")]
#[test_case(Mode::Basic, "blanket_impl", "", "changes_inner")]
#[test_case(Mode::Dynamic, "blanket_impl", "", "changes_inner")]
#[test_case(Mode::Static, "blanket_impl", "", "changes_inner")]
#[test_case(Mode::Basic, "closure", "", "changes_inner")]
#[test_case(Mode::Dynamic, "closure", "", "changes_inner")]
#[test_case(Mode::Static, "closure", "", "changes_inner")]
#[test_case(Mode::Basic, "closure", "", "changes_outer")]
#[test_case(Mode::Dynamic, "closure", "", "changes_outer")]
#[test_case(Mode::Static, "closure", "", "changes_outer")]
#[test_case(Mode::Basic, "closure", "", "changes_fn_ptr")]
#[test_case(Mode::Dynamic, "closure", "", "changes_fn_ptr")]
#[test_case(Mode::Static, "closure", "", "changes_fn_ptr")]
#[test_case(Mode::Basic, "closure", "", "changes_dyn")]
#[test_case(Mode::Dynamic, "closure", "", "changes_dyn")]
#[test_case(Mode::Static, "closure", "", "changes_dyn")]
#[test_case(Mode::Basic, "fn_ptr", "test_indirect", "test_indirect,changes_static")]
#[test_case(
    Mode::Dynamic,
    "fn_ptr",
    "test_indirect",
    "test_indirect,changes_static"
)]
#[test_case(
    Mode::Static,
    "fn_ptr",
    "test_indirect",
    "test_indirect,changes_static"
)]
#[test_case(Mode::Basic, "unused_lifetime", "", "changes_unused")]
#[test_case(Mode::Dynamic, "unused_lifetime", "", "changes_unused")]
#[test_case(Mode::Static, "unused_lifetime", "", "changes_unused")]
#[test_case(Mode::Basic, "doctests", "", "changes_no_run")]
#[test_case(Mode::Dynamic, "doctests", "", "changes_no_run")]
#[test_case(Mode::Static, "doctests", "", "changes_no_run")]
#[test_case(Mode::Basic, "doctests", "", "changes_compile_fail")]
#[test_case(Mode::Dynamic, "doctests", "", "changes_compile_fail")]
#[test_case(Mode::Static, "doctests", "", "changes_compile_fail")]
#[test_case(Mode::Basic, "doctests", "", "changes_run")]
#[test_case(Mode::Dynamic, "doctests", "", "changes_run")]
#[test_case(Mode::Static, "doctests", "", "changes_run")]
#[test_case(Mode::Basic, "doctests", "", "changes_main")]
#[test_case(Mode::Dynamic, "doctests", "", "changes_main")]
#[test_case(Mode::Static, "doctests", "", "changes_main")]
#[test_case(Mode::Basic, "doctests", "", "changes_should_panic")]
#[test_case(Mode::Dynamic, "doctests", "", "changes_should_panic")]
#[test_case(Mode::Static, "doctests", "", "changes_should_panic")]
#[test_case(Mode::Basic, "doctests", "", "changes_indirect_run")]
#[test_case(Mode::Dynamic, "doctests", "", "changes_indirect_run")]
#[test_case(Mode::Static, "doctests", "", "changes_indirect_run")]
#[test_case(Mode::Basic, "doctests", "", "changes_indirect_main")]
#[test_case(Mode::Dynamic, "doctests", "", "changes_indirect_main")]
#[test_case(Mode::Static, "doctests", "", "changes_indirect_main")]
#[test_case(Mode::Basic, "doctests", "", "changes_indirect_should_panic")]
#[test_case(Mode::Dynamic, "doctests", "", "changes_indirect_should_panic")]
#[test_case(Mode::Static, "doctests", "", "changes_indirect_should_panic")]
fn blackbox_test_affected(mode: Mode, name: &str, features_baseline: &str, features_changes: &str) {
    let mut dir = PATH.clone();
    dir.push(name);

    let target_dir = TempDir::new_in(
        env!("CARGO_TARGET_TMPDIR"),
        dir.file_name().unwrap().to_str().unwrap(),
    )
    .unwrap();

    {
        println!("-------- baseline --------");
        let result = command(&mode, &dir, target_dir.path(), Some(features_baseline))
            .output()
            .unwrap();
        println!("Stdout: {}", String::from_utf8(result.stdout).unwrap());
        println!("Stderr: {}", String::from_utf8(result.stderr).unwrap());
        assert!(result.status.success());
    }

    {
        println!("-------- with changes --------");
        let result = command(&mode, &dir, target_dir.path(), Some(features_changes))
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
#[test_case(
    Mode::Dynamic,
    "threading",
    "test2_panic",
    "test2_panic, changes_test1"
)]
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
