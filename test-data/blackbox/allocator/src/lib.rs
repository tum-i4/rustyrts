use mimalloc::MiMalloc;

const REFERENCE: &str = "The quick brown fox jumps over the lazy dog.";

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

#[test]
fn test() {
    assert_eq!(REFERENCE, REFERENCE.to_string());
}