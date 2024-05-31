use fs_utils::{get_cache_path, write_to_file, CacheFileDescr, CacheFileKind, CacheKind};
use std::borrow::Cow;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::AtomicPtr;
use std::sync::atomic::Ordering;

#[cfg(unix)]
use std::fs::read_to_string;

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
pub fn post_test(test_name: &str, append: bool) {
    let traces = reset_list();

    let file_descr = CacheFileDescr::new(test_name, None, None, CacheFileKind::Traces);
    export_traces(traces, |path_buf| file_descr.apply(path_buf), append);
}

#[no_mangle]
#[cfg(unix)]
pub fn post_main() {
    use std::os::unix::process::parent_id;

    let traces = read_list();

    let ppid = format!("{}", parent_id());
    let file_descr = CacheFileDescr::new(&ppid, None, None, CacheFileKind::ProcessTraces);
    export_traces(traces, |path_buf| file_descr.apply(path_buf), true);
}

#[cfg(unix)]
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

    #[cfg(unix)]
    let traces = {
        use std::process::id;

        let mut traces = traces;
        let pid = format!("{}", id());
        let mut path_child_traces = path_buf.clone();
        let file_descr = CacheFileDescr::new(&pid, None, None, CacheFileKind::ProcessTraces);
        file_descr.apply(&mut path_child_traces);
        if path_child_traces.is_file() {
            read_to_string(path_child_traces)
                .unwrap()
                .lines()
                .for_each(|l| {
                    traces.insert(Cow::Owned(l.to_string()));
                });
        }
        traces
    };

    let output = traces.into_iter().fold(String::new(), |mut acc, node| {
        acc.push_str(&node);
        acc.push('\n');
        acc
    });

    #[cfg(all(unix, feature = "fs_lock_syscall"))]
    use fs_utils::append_to_file;

    #[cfg(all(unix, feature = "fs_lock_syscall"))]
    if append {
        append_to_file(output, path_buf, path_buf_init);
    } else {
        write_to_file(output, path_buf, path_buf_init, false);
    }

    #[cfg(windows)] // TODO: fix file system races
    write_to_file(output, path_buf, path_buf_init, append);
}
