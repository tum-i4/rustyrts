use fs_utils::{get_dynamic_path, get_process_traces_path, get_traces_path, write_to_file};
use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::atomic::AtomicPtr;
use std::sync::atomic::Ordering;
use std::{collections::HashSet, fs::read_to_string};

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
                if let Ok(_) = traced.1.fetch_update(
                    Ordering::AcqRel,
                    Ordering::Acquire,
                    |supposed_to_be_max| {
                        if supposed_to_be_max as u64 == u64::MAX {
                            Some(prev)
                        } else {
                            None
                        }
                    },
                ) {
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
        |path_buf| get_traces_path(path_buf, test_name),
        false,
    );
}

#[no_mangle]
#[cfg(unix)]
pub fn post_main() {
    use std::os::unix::process::parent_id;

    let traces = read_list();

    let ppid = parent_id();
    export_traces(
        traces,
        |path_buf| get_process_traces_path(path_buf, &ppid),
        true,
    );
}

fn read_list<'a>() -> HashSet<Cow<'a, str>> {
    let mut traces = HashSet::new();

    let mut ptr = LIST.load(Ordering::Acquire);
    while let Some(traced) = unsafe { ptr.as_ref() } {
        traces.insert(Cow::Borrowed(traced.0.clone()));
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
        traces.insert(Cow::Borrowed((*name).clone()));
        ptr.store(u64::MAX as *mut Traced, Ordering::Release);
    }

    traces
}

fn export_traces<'a, F>(traces: HashSet<Cow<'a, str>>, path_buf_init: F, append: bool)
where
    F: FnOnce(PathBuf) -> PathBuf,
{
    let path_buf = get_dynamic_path(true);
    let mut traces = traces;

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
                    traces.insert(Cow::Owned(l.to_string()));
                });
        }
    }

    let output = traces.into_iter().fold(String::new(), |mut acc, node| {
        acc.push_str(&node);
        acc.push_str("\n");
        acc
    });

    write_to_file(output, path_buf, path_buf_init, append);
}
