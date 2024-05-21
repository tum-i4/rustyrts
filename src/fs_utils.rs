#![allow(dead_code)]

use std::fs::{read_to_string, DirEntry, OpenOptions};
use std::hash::Hash;
use std::io::Write;
use std::path::PathBuf;
use std::{collections::HashSet, convert::Into};

use crate::constants::{DIR_DOCTEST, DIR_DYNAMIC, DIR_STATIC, ENV_TARGET_DIR};

#[cfg(unix)]
use crate::constants::ENDING_PROCESS_TRACE;

pub enum CacheKind {
    Static,
    Dynamic,
    Doctests,
}

impl From<CacheKind> for &str {
    fn from(val: CacheKind) -> Self {
        match val {
            CacheKind::Static => DIR_STATIC,
            CacheKind::Dynamic => DIR_DYNAMIC,
            CacheKind::Doctests => DIR_DOCTEST,
        }
    }
}

pub fn get_cache_path(kind: CacheKind) -> Option<PathBuf> {
    let mut path_buf = PathBuf::from(std::env::var(ENV_TARGET_DIR).ok()?);
    path_buf.push(Into::<&str>::into(kind));
    Some(path_buf)
}

pub fn init_path(
    path_buf: &mut PathBuf,
    crate_name: &str,
    maybe_crate_id: Option<u64>,
    file_ending: &str,
) {
    if let Some(id) = maybe_crate_id {
        path_buf.push(format!("{}[{:016x}]", crate_name, id));
        path_buf.set_extension(file_ending);
    } else {
        path_buf.push(format!("{}", crate_name));
        path_buf.set_extension(file_ending);
    }
}

// pub fn get_graph_path(mut path_buf: PathBuf, crate_name: &str, id: u64) -> PathBuf {
//     path_buf.push(format!("{}[{:016x}]{}", crate_name, id, ENDING_GRAPH));
//     path_buf
// }

// pub fn get_test_path(mut path_buf: PathBuf, crate_name: &str, id: u64) -> PathBuf {
//     path_buf.push(format!("{}[{:016x}]{}", crate_name, id, ENDING_TEST));
//     path_buf
// }

// pub fn get_changes_path(mut path_buf: PathBuf, crate_name: &str, id: u64) -> PathBuf {
//     path_buf.push(format!("{}[{:016x}]{}", crate_name, id, ENDING_CHANGES));
//     path_buf
// }

// pub fn get_checksums_path(mut path_buf: PathBuf, crate_name: &str, id: u64) -> PathBuf {
//     path_buf.push(format!("{}[{:016x}]{}", crate_name, id, ENDING_CHECKSUM));
//     path_buf
// }

// pub fn get_checksums_vtbl_path(mut path_buf: PathBuf, crate_name: &str, id: u64) -> PathBuf {
//     path_buf.push(format!(
//         "{}[{:016x}]{}",
//         crate_name, id, ENDING_CHECKSUM_VTBL
//     ));
//     path_buf
// }

// pub fn get_checksums_const_path(mut path_buf: PathBuf, crate_name: &str, id: u64) -> PathBuf {
//     path_buf.push(format!(
//         "{}[{:016x}]{}",
//         crate_name, id, ENDING_CHECKSUM_CONST
//     ));
//     path_buf
// }

// pub fn get_dependencies_path(mut path_buf: PathBuf, test_name: &str) -> PathBuf {
//     path_buf.push(format!("{}{}", test_name, ENDING_DEPENDENCIES));
//     path_buf
// }

// pub fn get_traces_path(mut path_buf: PathBuf, test_name: &str) -> PathBuf {
//     path_buf.push(format!("{}{}", test_name, ENDING_TRACE));
//     path_buf
// }

#[cfg(unix)]
pub fn get_process_traces_path(mut path_buf: PathBuf, pid: &u32) -> PathBuf {
    path_buf.push(format!("{}{}", pid, ENDING_PROCESS_TRACE));
    path_buf
}

pub fn read_lines(files: &Vec<DirEntry>, file_ending: &str) -> HashSet<String>
where {
    read_lines_filter_map(files, file_ending, |_x| true, |x| x)
}

pub fn read_lines_filter_map<F, M, O>(
    files: &Vec<DirEntry>,
    file_ending: &str,
    filter: F,
    mapper: M,
) -> HashSet<O>
where
    F: Fn(&String) -> bool,
    M: std::ops::FnMut(std::string::String) -> O,
    O: Eq + Hash,
{
    let tokens: HashSet<O> = files
        .iter()
        .filter(|path| path.file_name().to_str().unwrap().ends_with(file_ending))
        .flat_map(|path| {
            let content = read_to_string(path.path()).unwrap();
            let lines: Vec<String> = content.split("\n").map(|s| s.to_string()).collect();
            lines
        })
        .filter(|line| !line.is_empty())
        .filter(filter)
        .map(mapper)
        .collect();
    tokens
}

/// Computes the location of a file from a closure
/// and overwrites the content of this file
///
/// ## Arguments
/// * `content` - new content of the file
/// * `path_buf` - `PathBuf` that points to the parent directory
/// * `initializer` - function that modifies path_buf - candidates: `get_graph_path`, `get_test_path`, `get_changes_path`
/// * 'append' - whether content should be appended
///
pub fn write_to_file<F, C: AsRef<[u8]>>(content: C, path_buf: PathBuf, initializer: F, append: bool)
where
    F: FnOnce(&mut PathBuf),
{
    let mut path_buf = path_buf;
    initializer(&mut path_buf);

    let mut file = match OpenOptions::new()
        .write(true)
        .append(append)
        .truncate(!append)
        .create(true)
        .open(path_buf.as_path())
    {
        Ok(file) => file,
        Err(reason) => panic!("Failed to create file: {}", reason),
    };

    match file.write_all(content.as_ref()) {
        Ok(_) => {}
        Err(reason) => panic!("Failed to write to file: {}", reason),
    };
}

/// Computes the location of a file from a closure
/// and links to this file
///
/// ## Arguments
/// * `path_orig` - `PathBuf` that points to the source directory
/// * `path_buf` - `PathBuf` that points to the parent directory
/// * `initializer` - function that modifies both paths - candidates: `get_graph_path`, `get_test_path`, `get_changes_path`
///
pub fn link_to_file<F>(path_orig: PathBuf, path_buf: PathBuf, initializer: F)
where
    F: Fn(&mut PathBuf),
{
    let mut path_orig = path_orig;
    let mut path_buf = path_buf;
    initializer(&mut path_orig);
    initializer(&mut path_buf);

    let _ = std::fs::remove_file(&path_buf);
    std::fs::hard_link(path_orig, path_buf).expect("Failed to create hard link");
}
