use std::ffi::OsString;
use std::fs::{copy, read_dir, DirEntry};
use std::path::Path;
use std::process::Command;

fn cargo() -> Command {
    Command::new(std::env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo")))
}

fn main() {
    println!("cargo:warning=Building rustyrts-dynamic-rlib");

    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();

    let mut cmd = cargo();
    cmd.current_dir(&format!("{}/rustyrts-dynamic-rlib", dir));
    cmd.arg("build");

    match cmd.status() {
        Ok(exit) => {
            if !exit.success() {
                std::process::exit(exit.code().unwrap_or(42));
            }
        }
        Err(ref e) => panic!("error while building rustyrts-dynamic-rlib: {:?}", e),
    }

    println!("cargo:warning=Installing rustyrts-dynamic-rlib");

    let files: Vec<DirEntry> =
        read_dir(format!("{}/rustyrts-dynamic-rlib/target/debug/deps/", dir))
            .unwrap()
            .filter(|res| res.is_ok())
            .map(|res| res.unwrap())
            .collect();

    let rlib_file = find_file(".rlib", &files);
    let rmeta_file = find_file(".rmeta", &files);
    let d_file = find_file(".d", &files);

    let cargo_home = std::env::var("CARGO_HOME").unwrap_or("~/.cargo".to_string());

    if let Some(entry) = rlib_file {
        let src = entry.path();
        let dst_str = format!("{}/bin/librustyrts_dynamic_rlib.rlib", cargo_home);
        let dst = Path::new(&dst_str);
        copy(src, dst).expect("Error while installing rustyrts-dynamic-rlib");
    }

    if let Some(entry) = rmeta_file {
        let src = entry.path();
        let dst_str = format!("{}/bin/librustyrts_dynamic_rlib.rmeta", cargo_home);
        let dst = Path::new(&dst_str);
        copy(src, dst).expect("Error while installing rustyrts-dynamic-rlib");
    }

    if let Some(entry) = d_file {
        let src = entry.path();
        let dst_str = format!("{}/bin/librustyrts_dynamic_rlib.d", cargo_home);
        let dst = Path::new(&dst_str);
        copy(src, dst).expect("Error while installing rustyrts-dynamic-rlib");
    }

    println!("cargo:rerun-if-changed=rustyrts-dynamic-rlib");
}

fn find_file<'a>(ends_with: &str, files: &'a Vec<DirEntry>) -> Option<&'a DirEntry> {
    files.into_iter().find(|file| {
        if file.file_name().to_str().unwrap().ends_with(ends_with) {
            return true;
        }
        false
    })
}
