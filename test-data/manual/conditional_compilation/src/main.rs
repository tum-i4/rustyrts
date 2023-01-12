static mut VAR: bool = false;

pub fn main() {}

#[cfg(foo)]
pub fn maybe_fn() {
    unsafe {
        VAR = true;
    }
}

#[cfg(not(foo))]
pub fn maybe_fn() {}

pub mod test {
    use crate::{maybe_fn, VAR};

    /// Test is only present if foo is set
    #[test]
    #[cfg(foo)]
    pub fn maybe_test() {
        assert!(true)
    }

    /// Test fails when foo is not set
    #[test]
    pub fn test_delegate() {
        maybe_fn();
        let value = unsafe { VAR };
        assert!(value)
    }

    /// Test fails when foo is not set
    #[test]
    pub fn test_macro() {
        assert!(cfg!(foo))
    }
}
