use proc_macro_derive::Echo;

// The purpose of this crate is, to validate that RustyRTS functions in the context of derive and proc macros

pub trait Echo {
    fn echo() -> u32;
}

fn echoed() -> u32 {
    42 // TODO: change this and check if test_derive is recognized as affected
}

#[derive(Echo)]
pub struct Foo;

#[cfg(test)]
mod test {
    use super::*;

    /// This test will fail if the value in `echoed()` is changed
    #[test]
    fn test_derive() {
        assert_eq!(Foo::echo(), 42);
    }
}
