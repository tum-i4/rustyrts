//########
// The purpose of this crate is, to ensure that dynamic RustyRTS collects traces of child processes

// This function has to be present in the traces of both tests foo and bar
pub fn library_fn() -> u8 {
    42
}
