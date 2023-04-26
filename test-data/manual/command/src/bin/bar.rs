use command::library_fn;

#[cfg(unix)]
pub fn main() {
    library_fn();
    library_fn();
    library_fn();
    library_fn();

    delegate_exit(library_fn());
}

fn delegate_exit(code: u8) {
    std::process::exit(code as i32);
}
