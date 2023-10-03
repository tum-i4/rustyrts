use constants::ENV_TARGET_DIR_OVERRIDE;
use fs_utils::{get_dynamic_path, get_process_traces_path, get_traces_path, write_to_file};
use std::hash::Hash;
use std::path::PathBuf;
use std::sync::Mutex;
use std::{collections::HashSet, fs::read_to_string};

use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::{AcqRel, Acquire, Release};

mod constants;
mod fs_utils;

static NODES: Mutex<Option<HashSet<Traced>>> = Mutex::new(None);

//######################################################################################################################
// Newtype tuple to specify Hash, PartialEq and Eq

struct Traced(&'static str, &'static u8);

impl Hash for Traced {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        (self.1 as *const u8 as usize).hash(state);
    }
}

impl PartialEq for Traced {
    fn eq(&self, other: &Self) -> bool {
        self.1 as *const u8 as usize == other.1 as *const u8 as usize
    }
}

impl Eq for Traced {}

//######################################################################################################################
// Functions for tracing

pub fn trace(input: &'static str, bit: &'static u8) {
    // SAFETY: We are given a reference to a u8 which has the same memory representation as bool,
    // and therefore also AtomicBool.
    let flag: &'static AtomicBool = unsafe { std::mem::transmute(bit) };

    if !flag.load(Acquire) {
        if !flag.fetch_or(true, AcqRel) {
            let mut handle = NODES.lock().unwrap();
            if let Some(ref mut set) = *handle {
                set.insert(Traced(input, bit));
            }
        }
    }
}

// pub fn trace_dyn<const N: usize>() {
//     println!("Intercepted virtual call at {:?}", N);
// }

pub fn pre_test() {
    std::env::remove_var(ENV_TARGET_DIR_OVERRIDE);
    let mut handle = NODES.lock().unwrap();
    if let Some(set) = handle.replace(HashSet::new()) {
        set.into_iter().for_each(|Traced(_, bit)| {
            // Reset bit-flag

            // SAFETY: We are given a reference to a u8 which has the same memory representation as bool,
            // and therefore also AtomicBool.
            let flag: &'static AtomicBool = unsafe { std::mem::transmute(bit) };
            flag.store(false, Release);
        });
    }
}

#[no_mangle]
#[cfg(unix)]
pub fn pre_main() {
    std::env::remove_var(ENV_TARGET_DIR_OVERRIDE);
    // Do not overwrite the HashSet in case it is present
    // This may be the case if main() is called directly by a test fn
    let mut handle = NODES.lock().unwrap();
    if handle.is_none() {
        *handle = Some(HashSet::new());
    }
}

#[no_mangle]
pub fn post_test(test_name: &str) {
    export_traces(|path_buf| get_traces_path(path_buf, test_name), false);
}

#[no_mangle]
#[cfg(unix)]
pub fn post_main() {
    use std::os::unix::process::parent_id;

    let ppid = parent_id();
    export_traces(|path_buf| get_process_traces_path(path_buf, &ppid), true);
}

pub fn export_traces<F>(path_buf_init: F, append: bool)
where
    F: FnOnce(PathBuf) -> PathBuf,
{
    let handle = NODES.lock().unwrap();
    if let Some(ref set) = *handle {
        let path_buf = get_dynamic_path(true, Some(ENV_TARGET_DIR_OVERRIDE));

        let mut all = HashSet::new();

        set.iter().for_each(|Traced(node, _)| {
            // Append node to acc
            all.insert(node.to_string());
        });

        #[cfg(unix)]
        {
            use std::process::id;

            let pid = id();
            let path_child_traces = get_process_traces_path(path_buf.clone(), &pid);
            if path_child_traces.is_file() {
                read_to_string(path_child_traces)
                    .unwrap()
                    .lines()
                    .for_each(|l| {
                        all.insert(l.to_string());
                    });
            }
        }

        let output = all.into_iter().fold(String::new(), |mut acc, node| {
            acc.push_str(&node);
            acc.push_str("\n");
            acc
        });

        write_to_file(output, path_buf, path_buf_init, append);
    }
}
