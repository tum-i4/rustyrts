#[macro_use]
extern crate lazy_static as foo;

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

    #[test]
    pub fn test_lazy_static() {
        assert_eq!(*VAR, 20)
    }
}
