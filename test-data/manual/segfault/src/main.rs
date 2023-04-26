fn main() {
    println!("Hello, world!");
}

#[test]
fn test() {
    unsafe { std::ptr::null_mut::<i32>().write(42) };
}
