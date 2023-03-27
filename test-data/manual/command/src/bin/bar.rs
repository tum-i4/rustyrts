use command::library_fn;

#[cfg(unix)]
pub fn main() {
    library_fn();
    library_fn();
    library_fn();
    library_fn();

    delegate_exit();
}

fn delegate_exit() {
    std::process::exit(42);
}
