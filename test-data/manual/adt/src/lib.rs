use std::fmt::{Debug, Display, Write};

struct Foo<T> {
    data: T,
}

impl Display for Foo<i32> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!("Foo: {}", self.data))
    }
}

impl Debug for Foo<u32> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!("Foo: {}", self.data))
    }
}

#[test]
fn test_display() {
    let mut instance: Foo<i32> = Foo { data: 1 };
    print!("{}", instance);
    assert_eq!(instance.data, 1);
}

#[test]
fn test_debug() {
    let mut instance: Foo<u32> = Foo { data: 1 };
    print!("{:?}", instance);
    assert_eq!(instance.data, 1);
}
