#![feature(trait_upcasting)]

// The purpose of this crate is, to verify if RustyRTS is able to recognize changes in functions that are called via dynamic dispatch

pub trait Foo {
    fn foo(&self) -> i32 {
        return 42;
    }
}

pub trait Bar: Foo {
    fn bar(&self) -> i32 {
        return 42;
    }
}

pub trait Baz: Bar {
    fn baz(&self) -> i32 {
        return 42;
    }
}

pub struct ImplFoo {}

pub struct ImplBaz {}

//#############
// TODO: When any of these four functions, that are called via dynamic dispatch, are commented in or out,
// test_dyn will fail and has to be recognized as affected

impl Foo for ImplFoo {
    //fn foo(&self) -> i32 {
    //    return 41;
    //}
}

impl Foo for ImplBaz {
    //fn foo(&self) -> i32 {
    //    return 41;
    //}
}

impl<T: Foo + ?Sized> Bar for T {
    //fn bar(&self) -> i32 {
    //    return 41;
    //}
}

// If this is uncommented, also test_static will fail and should be affected
impl Baz for ImplBaz {
    //fn baz(&self) -> i32 {
    //    return 41;
    //}
}

fn main() {
    println!("Hello, world!");
}

#[test]
pub fn test_dyn() {
    let bar: &dyn Bar = &ImplFoo {};
    let foo: &dyn Foo = bar; // Up-casting from Bar to Foo (only possible with special compiler feature)

    assert_eq!(bar.foo(), 42);

    assert_eq!(foo.foo(), 42);
    assert_eq!(bar.bar(), 42);

    let baz: &dyn Baz = &ImplBaz {};
    assert_eq!(baz.foo(), 42);
    assert_eq!(baz.bar(), 42);
    assert_eq!(baz.baz(), 42);
}

#[test]
pub fn test_static() {
    let impl_baz = ImplBaz {};
    assert_eq!(impl_baz.baz(), 42);
}
