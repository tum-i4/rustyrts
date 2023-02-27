use std::collections::HashSet;
use std::env;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::RwLock;

static mut NODES: RwLock<Option<HashSet<&'static str>>> = RwLock::new(None);

//######################################################################################################################
// Functions for tracing during RTS

#[no_mangle]
#[inline]
pub fn trace(input: &'static str) {
    let mut handle = unsafe { NODES.write() }.unwrap();
    if let Some(ref mut set) = *handle {
        if set.get(input).is_none() {
            set.insert(input);
        }
    }
}

#[no_mangle]
#[inline]
pub fn trace_atomic(input: &'static str, bit: &'static AtomicBool) {
    if bit.fetch_or(false, SeqCst) {
        let mut handle = unsafe { NODES.write() }.unwrap();
        if let Some(ref mut set) = *handle {
            if set.get(input).is_none() {
                set.insert(input);
            }
        }
    }
}

#[no_mangle]
#[inline]
pub fn pre_processing() {
    let mut handle = unsafe { NODES.write() }.unwrap();
    if let Some(ref mut set) = *handle {
        set.clear();
    } else {
        *handle = Some(HashSet::new());
    }
}

#[no_mangle]
#[inline]
pub fn post_processing(test_name: &str) {
    let handle = unsafe { NODES.read() }.unwrap();
    if let Some(ref set) = *handle {
        let source_path = env::var("PROJECT_DIR").unwrap();
        let mut path_buf = PathBuf::from_str(&source_path).unwrap();
        path_buf.push(".rts_dynamic");
        let vec: Vec<&'static str> = set.iter().cloned().collect();
        write_to_file(vec.join("\n").to_string(), path_buf, |buf| {
            get_traces_path(buf, &test_name)
        });
    }
}

//######################################################################################################################
// Auxiliary functions

// TODO: This is copied code:

pub fn get_traces_path(mut path_buf: PathBuf, test_name: &str) -> PathBuf {
    path_buf.push(format!("{}.trace", test_name));
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
