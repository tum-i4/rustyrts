use std::fmt::{Display, Write};

struct Foo {
    data: i32,
}

impl Display for Foo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!("Foo: {}", self.data))
    }
}

#[test]
fn test() {
    let mut instance: Foo = Foo { data: 1 };
    print!("{}", instance);
    assert_eq!(instance.data, 0);
}
