use std::fmt::Display;

struct Foo;
struct Bar;
struct Baz;


impl Display for Foo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Foo")
    }
}


impl Display for Bar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Bar")
    }
}

impl Display for Baz {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Bar")
    }
}


fn main() {
    println!("Hello, world!");
}

#[test]
fn test_foo() {
    assert_eq!(format!("{}", Foo), "Foo")
}

#[test]
fn test_bar() {
    assert_eq!(format!("{}", Bar), "Bar")
}

#[test]
fn test_baz() {
    assert_eq!(format!("{}", Baz), "Baz")
}
