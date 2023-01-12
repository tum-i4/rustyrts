use std::path::PathBuf;
use std::str::FromStr;

pub fn get_base_path(str: &str) -> PathBuf {
    let mut path_buf = PathBuf::from_str(str).unwrap();
    path_buf.push(".rts");
    path_buf
}

pub fn get_graph_path(mut path_buf: PathBuf, crate_name: &str) -> PathBuf {
    path_buf.push(format!("{}.dot", crate_name,));
    path_buf
}

pub fn get_test_path(mut path_buf: PathBuf, crate_name: &str) -> PathBuf {
    path_buf.push(format!("{}.test", crate_name));
    path_buf
}

pub fn get_changes_path(mut path_buf: PathBuf, crate_name: &str) -> PathBuf {
    path_buf.push(format!("{}.changes", crate_name));
    path_buf
}
