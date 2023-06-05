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

fn generic_display<S: Display>(s: &S) {
    println!("{}", s);
}

trait DynFoo<T> {
    type Ty;

    fn data(&self) -> &T;
    fn type_data(&self) -> Self::Ty;
}

impl DynFoo<i32> for Foo<i32> {
    fn data(&self) -> &i32 {
        &self.data
    }

    type Ty = u32;

    fn type_data(&self) -> Self::Ty {
        self.data as u32
    }
}

impl Display for dyn DynFoo<i32, Ty = u32> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!("signed: {}", self.data()))?;
        f.write_str(&format!("unsigned: {}", self.type_data()))
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

#[test]
fn test_generic() {
    let mut instance: Foo<i32> = Foo { data: 1 };
    generic_display(&instance);
    assert_eq!(instance.data, 1);
}

#[test]
fn test_dyn() {
    let mut dyn_instance: Box<dyn DynFoo<i32, Ty = u32>> = Box::new(Foo { data: 1 });
    print!("{}", dyn_instance);
}
