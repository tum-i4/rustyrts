extern crate jemallocator;

fn main() {
    println!("Hello, world!");
}

#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;


#[test]
fn test() {
    let mut foo = String::new();
    foo.push_str("Foo");
    assert_eq!(foo, "Foo");
}
