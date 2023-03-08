use std::fs::read_to_string;
use std::str::FromStr;
use std::{collections::HashSet, path::PathBuf};

use libc::{c_int, waitpid, WEXITSTATUS, WIFEXITED};

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

pub fn waitpid_wrapper(pid: libc::pid_t) -> Result<c_int, String> {
    let mut status: c_int = 0;

    // SAFETY: Just a call to libc
    let res = unsafe { waitpid(pid, &mut status as *mut c_int, 0) };

    if res == pid {
        if WIFEXITED(status) {
            Ok(WEXITSTATUS(status))
        } else {
            Err(format!("Wrong status: {}", status))
        }
    } else {
        Err(format!("Joined wrong process: {}", res))
    }
}
