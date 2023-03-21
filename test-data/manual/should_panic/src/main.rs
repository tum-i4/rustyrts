fn main() {
    println!("Hello, world!");
}

fn func() {
    println!("Foo");
}

#[test]
#[should_panic]
fn test_should_panic() {
    func();
    panic!()
}
