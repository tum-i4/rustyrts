use std::collections::HashSet;
use std::fs::{read_to_string, DirEntry, File};
use std::hash::Hash;
use std::io::Write;
use std::path::PathBuf;
use std::str::FromStr;

use crate::constants::{
    ENDING_CHANGES, ENDING_CHECKSUM, ENDING_GRAPH, ENDING_TEST, ENDING_TRACE, FILE_AFFECTED,
};

pub fn get_static_path(str: &str) -> PathBuf {
    let mut path_buf = PathBuf::from_str(str).unwrap();
    path_buf.push(".rts_static");
    path_buf
}

pub fn get_dynamic_path(str: &str) -> PathBuf {
    let mut path_buf = PathBuf::from_str(str).unwrap();
    path_buf.push(".rts_dynamic");
    path_buf
}

pub fn get_graph_path(mut path_buf: PathBuf, crate_name: &str, id: u64) -> PathBuf {
    path_buf.push(format!("{}[{:16x}]{}", crate_name, id, ENDING_GRAPH));
    path_buf
}

pub fn get_test_path(mut path_buf: PathBuf, crate_name: &str, id: u64) -> PathBuf {
    path_buf.push(format!("{}[{:16x}]{}", crate_name, id, ENDING_TEST));
    path_buf
}

pub fn get_changes_path(mut path_buf: PathBuf, crate_name: &str, id: u64) -> PathBuf {
    path_buf.push(format!("{}[{:16x}]{}", crate_name, id, ENDING_CHANGES));
    path_buf
}

pub fn get_checksums_path(mut path_buf: PathBuf, crate_name: &str, id: u64) -> PathBuf {
    path_buf.push(format!("{}[{:16x}]{}", crate_name, id, ENDING_CHECKSUM));
    path_buf
}

pub fn get_traces_path(mut path_buf: PathBuf, test_name: &str) -> PathBuf {
    path_buf.push(format!("{}{}", test_name, ENDING_TRACE));
    path_buf
}

pub fn get_affected_path(mut path_buf: PathBuf) -> PathBuf {
    path_buf.push(FILE_AFFECTED);
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
    O: Eq + Hash + Ord,
{
    let tokens: HashSet<O> = files
        .iter()
        .filter(|path| path.file_name().to_str().unwrap().ends_with(file_ending))
        .flat_map(|path| {
            let content = read_to_string(path.path()).unwrap();
            let lines: Vec<String> = content.split("\n").map(|s| s.to_string()).collect();
            lines
        })
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
///
pub fn write_to_file<F>(content: String, path_buf: PathBuf, initializer: F)
where
    F: FnOnce(PathBuf) -> PathBuf,
{
    let path_buf = initializer(path_buf);
    let mut file = match File::create(path_buf.as_path()) {
        Ok(file) => file,
        Err(reason) => panic!("Failed to create file: {}", reason),
    };

    match file.write_all(content.as_bytes()) {
        Ok(_) => {}
        Err(reason) => panic!("Failed to write to file: {}", reason),
    };
}
