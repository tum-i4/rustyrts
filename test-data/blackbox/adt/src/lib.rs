#![allow(dead_code)]
use std::fmt::{Debug, Display, Write};

struct Foo<T> {
    data: T,
}

impl Display for Foo<i32> {
    #[cfg(not(feature = "changes_display"))]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!("Foo: {}", self.data))
    }

    #[cfg(feature = "changes_display")]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!("Unexpected: {}", self.data))
    }
}

impl Debug for Foo<u32> {
    #[cfg(not(feature = "changes_debug"))]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!("Foo: {}", self.data))
    }

    #[cfg(feature = "changes_debug")]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!("Unexpected: {}", self.data))
    }
}

static mut DROPPED: bool = false;

impl<T> Drop for Foo<T> {
    #[cfg(not(feature = "changes_drop"))]
    fn drop(&mut self) {
        unsafe { DROPPED = true };
    }

    #[cfg(feature = "changes_drop")]
    fn drop(&mut self) {}
}

fn generic_display<S: Display>(s: &S, buf: &mut impl Write) {
    buf.write_fmt(format_args!("{}", s)).unwrap();
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
        f.write_str(&format!(
            "signed: {}, unsigned: {}",
            self.data(),
            self.type_data()
        ))
    }
}

#[cfg(test)]
pub mod test {
    use crate::*;

    #[test]
    fn test_display() {
        {
            let instance: Foo<i32> = Foo { data: 1 };
            assert_eq!(format!("{}", instance), "Foo: 1");
        }
        assert!(unsafe { DROPPED });
    }

    #[test]
    fn test_debug() {
        let instance: Foo<u32> = Foo { data: 1 };
        assert_eq!(format!("{:?}", instance), "Foo: 1");
    }

    #[test]
    fn test_generic() {
        let mut buf = String::new();
        let instance: Foo<i32> = Foo { data: 1 };
        generic_display(&instance, &mut buf);
        assert_eq!(buf, "Foo: 1")
    }

    #[test]
    fn test_dyn() {
        let dyn_instance: Box<dyn DynFoo<i32, Ty = u32>> = Box::new(Foo { data: -1 });
        assert_eq!(
            format!("{}", dyn_instance),
            "signed: -1, unsigned: 4294967295"
        );
    }
}
