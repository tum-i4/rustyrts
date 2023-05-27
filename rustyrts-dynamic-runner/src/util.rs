#![cfg(unix)]

use libc::{c_int, waitpid, WEXITSTATUS, WIFEXITED};

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
