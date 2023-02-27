use proc_macro_derive::Echo;

pub trait Echo {
    fn echo() -> u32;
}

fn echoed() -> u32 {
    42
}

#[derive(Echo)]
pub struct Foo;

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_derive() {
        assert_eq!(Foo::echo(), 42);
    }
}
