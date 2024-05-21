use constants::ENDING_TRACE;
use fs_utils::{get_cache_path, get_process_traces_path, init_path, write_to_file, CacheKind};
use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::atomic::AtomicPtr;
use std::sync::atomic::Ordering;
use std::{collections::HashSet, fs::read_to_string};

use crate::constants::ENDING_PROCESS_TRACE;

mod constants;
mod fs_utils;

static LIST: AtomicPtr<Traced> = AtomicPtr::new(std::ptr::null::<Traced>() as *mut Traced);

//######################################################################################################################
// Tuple type for tracing

struct Traced(&'static str, AtomicPtr<Traced>);

//##########>############################################################################################################
// Functions for tracing

#[no_mangle]
pub fn trace(input: &'static mut (&str, usize)) {
    let traced: &mut Traced = unsafe { std::mem::transmute(input) };

    if traced.1.load(Ordering::Acquire) as u64 == u64::MAX {
        // Append to list
        LIST.fetch_update(Ordering::AcqRel, Ordering::Acquire, |prev| {
            if traced
                .1
                .fetch_update(Ordering::AcqRel, Ordering::Acquire, |supposed_to_be_max| {
                    if supposed_to_be_max as u64 == u64::MAX {
                        Some(prev)
                    } else {
                        None
                    }
                })
                .is_ok()
            {
                Some(traced)
            } else {
                None
            }
        })
        .map(|_| ())
        .unwrap_or_default();
    }
}

pub fn pre_test() {
    reset_list();
}

#[no_mangle]
#[cfg(unix)]
pub fn pre_main() {}

#[no_mangle]
pub fn post_test(test_name: &str) {
    let traces = reset_list();
    export_traces(
        traces,
        |path_buf| init_path(path_buf, test_name, None, ENDING_TRACE),
        false,
    );
}

#[no_mangle]
#[cfg(unix)]
pub fn post_main() {
    use std::os::unix::process::parent_id;

    use crate::constants::ENDING_PROCESS_TRACE;

    let traces = read_list();

    let ppid = parent_id();
    export_traces(
        traces,
        |path_buf| init_path(path_buf, &format!("{}", ppid), None, ENDING_PROCESS_TRACE),
        true,
    );
}

fn read_list<'a>() -> HashSet<Cow<'a, str>> {
    let mut traces = HashSet::new();

    let mut ptr = LIST.load(Ordering::Acquire);
    while let Some(traced) = unsafe { ptr.as_ref() } {
        traces.insert(Cow::Borrowed(traced.0));
        ptr = traced.1.load(Ordering::Acquire);
    }

    traces
}

fn reset_list<'a>() -> HashSet<Cow<'a, str>> {
    let mut traces = HashSet::new();

    while let Ok(prev) = LIST.fetch_update(Ordering::AcqRel, Ordering::Acquire, |prev| {
        let Traced(_str, next_ptr) = unsafe { prev.as_ref() }?;
        Some(next_ptr.load(Ordering::Acquire))
    }) {
        let Traced(name, ptr) = unsafe { prev.as_ref() }.unwrap();
        traces.insert(Cow::Borrowed(*name));
        ptr.store(u64::MAX as *mut Traced, Ordering::Release);
    }

    traces
}

fn export_traces<F>(traces: HashSet<Cow<'_, str>>, path_buf_init: F, append: bool)
where
    F: FnOnce(&mut PathBuf),
{
    let path_buf = get_cache_path(CacheKind::Dynamic).unwrap();
    let mut traces = traces;

    #[cfg(unix)]
    {
        use std::process::id;
        let pid = id();
        let mut path_child_traces = path_buf.clone();
        init_path(
            &mut path_child_traces,
            &format!("{}", pid),
            None,
            ENDING_PROCESS_TRACE,
        );
        if path_child_traces.is_file() {
            read_to_string(path_child_traces)
                .unwrap()
                .lines()
                .for_each(|l| {
                    traces.insert(Cow::Owned(l.to_string()));
                });
        }
    }

    let output = traces.into_iter().fold(String::new(), |mut acc, node| {
        acc.push_str(&node);
        acc.push('\n');
        acc
    });

    write_to_file(output, path_buf, path_buf_init, append);
}
