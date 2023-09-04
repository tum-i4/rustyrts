use lazy_static::lazy_static;

// The purpose of this crate is, to verify that RustyRTS can handle variables that are initialized lazily

// TODO: change the argument
lazy_static! {
    static ref VAR: usize = value(2);
}

fn value(input: usize) -> usize {
    input * 10
}

fn main() {}

#[cfg(test)]
pub mod test {

    use crate::VAR;

    /// This test will fail if the argument in `VAR` is changed
    #[test]
    pub fn test_lazy_static() {
        assert_eq!(*VAR, 20)
    }
}
