use std::path::PathBuf;
use std::str::FromStr;

pub fn get_base_path(str: &str) -> PathBuf {
    let mut path_buf = PathBuf::from_str(str).unwrap();
    path_buf.push(".rts");
    path_buf
}

pub fn get_graph_path(mut path_buf: PathBuf, crate_name: &str, id: u64) -> PathBuf {
    path_buf.push(format!("{}[{:16x}].dot", crate_name, id,));
    path_buf
}

pub fn get_test_path(mut path_buf: PathBuf, crate_name: &str, id: u64) -> PathBuf {
    path_buf.push(format!("{}[{:16x}].test", crate_name, id,));
    path_buf
}

pub fn get_changes_path(mut path_buf: PathBuf, crate_name: &str, id: u64) -> PathBuf {
    path_buf.push(format!("{}[{:16x}].changes", crate_name, id,));
    path_buf
}

pub fn get_checksums_path(mut path_buf: PathBuf, crate_name: &str, id: u64) -> PathBuf {
    path_buf.push(format!("{}[{:16x}].checksum", crate_name, id,));
    path_buf
}
