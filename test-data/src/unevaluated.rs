const BAZ: &str = "Baz";

mod test {
    use super::*;

    #[test]
    fn test_const_read() {
        assert_eq!(BAZ, "Baz");
    }
}
