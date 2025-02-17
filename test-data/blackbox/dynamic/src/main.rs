#![feature(trait_upcasting)]

// The purpose of this crate is, to verify if RustyRTS is able to recognize changes in functions that are called via dynamic dispatch

pub trait Foo {
    fn foo(&self) -> i32 {
        return 42;
    }
}

pub trait Bar: Foo {
    fn bar(&self) -> i32 {
        return 41;
    }
}

pub trait Baz: Bar {
    fn baz(&self) -> i32 {
        return 42;
    }
}

pub struct ImplFoo {}

pub struct ImplBar {}

pub struct ImplBaz {}

//#############
// When any of these four functions, that are called via dynamic dispatch, are included,
// test_dyn_added will fail and has to be recognized as affected

impl Foo for ImplFoo {
    #[cfg(feature = "changes_direct")]
    fn foo(&self) -> i32 {
        return 41;
    }
}

impl Foo for ImplBaz {
    #[cfg(feature = "changes_indirect")]
    fn foo(&self) -> i32 {
        return 41;
    }
}

// If this is included, also test_static will fail and should be affected
impl Baz for ImplBaz {
    #[cfg(feature = "changes_static")]
    fn baz(&self) -> i32 {
        return 41;
    }
}

//#############
// When this function, that is called via dynamic dispatch, is excluded,
// test_dyn_removed will fail and has to be recognized as affected

impl<T: Foo + ?Sized> Bar for T {
    #[cfg(not(feature = "changes_removed"))]
    fn bar(&self) -> i32 {
        return 42;
    }
}

fn main() {
    println!("Hello, world!");
}

#[test]
pub fn test_dyn_added() {
    let bar: &dyn Bar = &ImplFoo {};
    let foo: &dyn Foo = bar; // Up-casting from Bar to Foo (only possible with special compiler feature)

    assert_eq!(foo.foo(), 42);

    let baz: &dyn Baz = &ImplBaz {};
    assert_eq!(baz.foo(), 42);
    assert_eq!(baz.baz(), 42);
}

#[test]
pub fn test_dyn_removed() {
    let bar: &dyn Bar = &ImplFoo {};

    assert_eq!(bar.foo(), 42);
    assert_eq!(bar.bar(), 42);
}

#[test]
pub fn test_static() {
    let impl_baz = ImplBaz {};
    assert_eq!(impl_baz.baz(), 42);
}
