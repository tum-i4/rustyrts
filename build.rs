use std::ffi::OsString;
use std::fs::{copy, read_dir, DirEntry};
use std::path::PathBuf;
use std::process::Command;

fn cargo() -> Command {
    Command::new(std::env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo")))
}

fn main() {
    if std::env::var("RUSTYRTS_SKIP_BUILD").is_err() {
        build_library("rustyrts-dynamic-rlib");
        build_library("rustyrts-dynamic-runner");

        install_rlib("rustyrts_dynamic_rlib", "rustyrts-dynamic-rlib");
        install_staticlib("rustyrts_dynamic_runner", "rustyrts-dynamic-runner");
    }
}

fn build_library(dir_name: &str) {
    println!("cargo:warning=Building {}", dir_name);

    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();

    //let mut cmd = cargo();
    //let mut path = PathBuf::new();
    //path.push(dir);
    //path.push(dir_name);
    //cmd.current_dir(path);
    //cmd.arg("clean");
    //
    //match cmd.status() {
    //    Ok(exit) => {
    //        if !exit.success() {
    //            std::process::exit(exit.code().unwrap_or(42));
    //        }
    //    }
    //    Err(ref e) => panic!("error while cleaning {}: {:?}", dir_name, e),
    //}

    let mut cmd = cargo();
    let mut path = PathBuf::new();
    path.push(dir);
    path.push(dir_name);
    cmd.current_dir(path);
    cmd.arg("build");

    match cmd.status() {
        Ok(exit) => {
            if !exit.success() {
                std::process::exit(exit.code().unwrap_or(42));
            }
        }
        Err(ref e) => panic!("error while building {}: {:?}", dir_name, e),
    }

    println!("cargo:rerun-if-changed={}", dir_name);
}

fn install_rlib(name: &str, dir_name: &str) {
    println!("cargo:warning=Installing {}", name);

    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();

    let mut path = PathBuf::new();
    path.push(dir);
    path.push(dir_name);
    path.push("target");
    path.push("debug");
    //path.push("deps");

    let files: Vec<DirEntry> = read_dir(path)
        .unwrap()
        .filter(|res| res.is_ok())
        .map(|res| res.unwrap())
        .collect();

    let rlib_file = find_file(&format!("lib{}", name), ".rlib", &files);
    //let rmeta_file = find_file(&format!("lib{}", name), ".rmeta", &files);
    let d_file = find_file(name, ".d", &files);

    let mut cargo_home = {
        let maybe_cargo_home = std::env::var("CARGO_HOME");
        if let Ok(cargo_home) = maybe_cargo_home {
            PathBuf::from(cargo_home)
        } else {
            let home = std::env::var("HOME").expect("Unable to find HOME environment variable");
            let mut path = PathBuf::new();
            path.push(home);
            path.push(".cargo");
            path
        }
    };
    cargo_home.push("bin");

    if let Some(entry) = rlib_file {
        let src = entry.path();
        let mut dst = cargo_home.clone();
        dst.push(format!("lib{}.rlib", name));
        copy(src, dst).expect(&format!("Error while installing {}", name));
    }

    //if let Some(entry) = rmeta_file {
    //    let src = entry.path();
    //    let mut dst = cargo_home.clone();
    //    dst.push(format!("lib{}.rmeta", name));
    //    copy(src, dst).expect(&format!("Error while installing {}", name));
    //}

    if let Some(entry) = d_file {
        let src = entry.path();
        let mut dst = cargo_home.clone();
        dst.push(format!("{}.d", name));
        copy(src, dst).expect(&format!("Error while installing {}", name));
    }
}

fn install_staticlib(name: &str, dir_name: &str) {
    println!("cargo:warning=Installing {}", name);

    let mut dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    dir.push(dir_name);
    dir.push("target");
    dir.push("debug");
    //dir.push("deps");

    let files: Vec<DirEntry> = read_dir(dir)
        .unwrap()
        .filter(|res| res.is_ok())
        .map(|res| res.unwrap())
        .collect();

    let a_file = find_file(&format!("lib{}", name), ".a", &files);
    let d_file = find_file(name, ".d", &files);

    let mut cargo_home = {
        let maybe_cargo_home = std::env::var("CARGO_HOME");
        if let Ok(cargo_home) = maybe_cargo_home {
            PathBuf::from(cargo_home)
        } else {
            let home = std::env::var("HOME").expect("Unable to find HOME environment variable");
            let mut path = PathBuf::new();
            path.push(home);
            path.push(".cargo");
            path
        }
    };
    cargo_home.push("bin");

    if let Some(entry) = a_file {
        let src = entry.path();
        let mut dst = cargo_home.clone();
        dst.push(format!("lib{}.a", name));
        copy(src, dst).expect(&format!("Error while installing {}", name));
    }

    if let Some(entry) = d_file {
        let src = entry.path();
        let mut dst = cargo_home.clone();
        dst.push(format!("{}.d", name));
        copy(src, dst).expect(&format!("Error while installing {}", name));
    }
}

fn find_file<'a>(
    starts_with: &str,
    ends_with: &str,
    files: &'a Vec<DirEntry>,
) -> Option<&'a DirEntry> {
    files.into_iter().find(|file| {
        let file_name_os = file.file_name();
        let file_name = file_name_os.to_str().unwrap();
        if file_name.starts_with(starts_with) && file_name.ends_with(ends_with) {
            return true;
        }
        false
    })
}
