use std::ffi::OsString;
use std::fs::{copy, read_dir, DirEntry};
use std::path::Path;
use std::process::Command;

fn cargo() -> Command {
    Command::new(std::env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo")))
}

fn main() {
    build_library("rustyrts-dynamic-rlib");
    build_library("rustyrts-dynamic-runner");

    install_rlib("rustyrts_dynamic_rlib", "rustyrts-dynamic-rlib");
    install_staticlib("rustyrts_dynamic_runner", "rustyrts-dynamic-runner");
}

fn build_library(dir_name: &str) {
    println!("cargo:warning=Building {}", dir_name);

    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();

    let mut cmd = cargo();
    cmd.current_dir(&format!("{}/{}", dir, dir_name));
    cmd.arg("clean");

    match cmd.status() {
        Ok(exit) => {
            if !exit.success() {
                std::process::exit(exit.code().unwrap_or(42));
            }
        }
        Err(ref e) => panic!("error while cleaning {}: {:?}", dir_name, e),
    }

    let mut cmd = cargo();
    cmd.current_dir(&format!("{}/{}", dir, dir_name));
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

    let files: Vec<DirEntry> = read_dir(format!("{}/{}/target/debug/deps", dir, dir_name))
        .unwrap()
        .filter(|res| res.is_ok())
        .map(|res| res.unwrap())
        .collect();

    let rlib_file = find_file(&format!("lib{}", name), ".rlib", &files);
    let rmeta_file = find_file(&format!("lib{}", name), ".rmeta", &files);
    let d_file = find_file(name, ".d", &files);

    let cargo_home = std::env::var("CARGO_HOME").unwrap_or("~/.cargo".to_string());

    if let Some(entry) = rlib_file {
        let src = entry.path();
        let dst_str = format!("{}/bin/lib{}.rlib", cargo_home, name);
        let dst = Path::new(&dst_str);
        copy(src, dst).expect(&format!("Error while installing {}", name));
    }

    if let Some(entry) = rmeta_file {
        let src = entry.path();
        let dst_str = format!("{}/bin/lib{}.rmeta", cargo_home, name);
        let dst = Path::new(&dst_str);
        copy(src, dst).expect(&format!("Error while installing {}", name));
    }

    if let Some(entry) = d_file {
        let src = entry.path();
        let dst_str = format!("{}/bin/{}.d", cargo_home, name);
        let dst = Path::new(&dst_str);
        copy(src, dst).expect(&format!("Error while installing {}", name));
    }
}

fn install_staticlib(name: &str, dir_name: &str) {
    println!("cargo:warning=Installing {}", name);

    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();

    let files: Vec<DirEntry> = read_dir(format!("{}/{}/target/debug/deps", dir, dir_name))
        .unwrap()
        .filter(|res| res.is_ok())
        .map(|res| res.unwrap())
        .collect();

    let a_file = find_file(&format!("lib{}", name), ".a", &files);
    let d_file = find_file(name, ".d", &files);

    let cargo_home = std::env::var("CARGO_HOME").unwrap_or("~/.cargo".to_string());

    if let Some(entry) = a_file {
        let src = entry.path();
        let dst_str = format!("{}/bin/lib{}.a", cargo_home, name);
        let dst = Path::new(&dst_str);
        copy(src, dst).expect(&format!("Error while installing {}", name));
    }

    if let Some(entry) = d_file {
        let src = entry.path();
        let dst_str = format!("{}/bin/{}.d", cargo_home, name);
        let dst = Path::new(&dst_str);
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
