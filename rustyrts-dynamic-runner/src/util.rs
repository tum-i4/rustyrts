use std::fs::read_to_string;
use std::str::FromStr;
use std::{collections::HashSet, path::PathBuf};

pub fn get_dynamic_path(str: &str) -> PathBuf {
    let mut path_buf = PathBuf::from_str(str).unwrap();
    path_buf.push(".rts_dynamic");
    path_buf
}

pub fn get_affected_path(mut path_buf: PathBuf) -> PathBuf {
    path_buf.push("affected");
    path_buf
}

pub fn read_lines(path_buf: PathBuf) -> HashSet<String> {
    let content = read_to_string(path_buf).unwrap();
    let lines: HashSet<String> = content.split("\n").map(|s| s.to_string()).collect();
    lines
}
