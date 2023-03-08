use core::panic;
use std::collections::HashSet;
use std::env;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::RwLock;

static NODES: RwLock<Option<HashSet<(&'static str, &'static u8)>>> = RwLock::new(None);

//######################################################################################################################
// Functions for tracing

#[no_mangle]
pub fn trace(input: &'static str, bit: &'static u8) {
    // SAFETY: We are given a reference to a u8 which has the same memory representation as bool,
    // and therefore also AtomicBool.
    let flag: &'static AtomicBool = unsafe { std::mem::transmute(bit) };
    if !flag.fetch_or(true, SeqCst) {
        let mut handle = NODES.write().unwrap();
        if let Some(ref mut set) = *handle {
            set.insert((input, bit));
        }
    }
}

#[no_mangle]
pub fn pre_processing() {
    let mut handle = NODES.write().unwrap();
    if let Some(set) = handle.replace(HashSet::new()) {
        set.into_iter().for_each(|(_, bit)| {
            // Reset bitflag
            let flag: &'static AtomicBool = unsafe { std::mem::transmute(bit) };
            flag.store(false, SeqCst);
        });
    }
}

#[no_mangle]
pub fn post_processing(test_name: &str) {
    let handle = NODES.read().unwrap();
    if let Some(ref set) = *handle {
        if let Ok(source_path) = env::var(ENV_PROJECT_DIR) {
            let mut path_buf = PathBuf::from_str(&source_path).unwrap();
            path_buf.push(DIR_DYNAMIC);
            let output = set.iter().fold(String::new(), |mut acc, (node, _)| {
                // Append node to acc
                acc.push_str(node);
                acc.push_str("\n");
                acc
            });
            write_to_file(output, path_buf, |buf| get_traces_path(buf, &test_name));
        }
    }
}

//######################################################################################################################
// Auxiliary functions

// TODO: This is copied code:

pub const ENV_PROJECT_DIR: &str = "PROJECT_DIR";
pub const DIR_DYNAMIC: &str = ".rts_dynamic";
pub const ENDING_TRACE: &str = ".trace";

pub fn get_traces_path(mut path_buf: PathBuf, test_name: &str) -> PathBuf {
    path_buf.push(format!("{}{}", test_name, ENDING_TRACE));
    path_buf
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
