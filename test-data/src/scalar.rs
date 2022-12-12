static FOO: &str = "Foo";
static mut BAR: &str = "Bar";

mod test {
    use super::*;

    #[test]
    fn test_direct_read() {
        assert_eq!(FOO, "Foo");
    }

    #[test]
    fn test_indirect_ptr_read() {
        let ptr = &FOO as *const &str;
        unsafe {
            assert_eq!(*ptr, "Foo");
        }
    }

    #[test]
    fn test_indirect_ref_read() {
        let ptr = &FOO;
        assert_eq!(*ptr, "Foo");
    }

    #[test]
    fn test_direct_write() {
        unsafe {
            BAR = "42";
        }
        assert!(true);
    }

    #[test]
    fn test_indirect_ptr_write() {
        unsafe {
            let ptr = &mut BAR as *mut &str;
            *ptr = "42";
        }
        assert!(true);
    }

    #[test]
    fn test_indirect_ref_write() {
        unsafe {
            let ptr = &mut BAR;
            *ptr = "42";
        }
        assert!(true);
    }

    #[test]
    fn test_mut_read() {
        unsafe {
            let ptr = &mut BAR;
            assert_eq!(*ptr, "Bar");
        }
    }
}
