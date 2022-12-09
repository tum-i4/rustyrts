fn func() -> u64 {
    42
}

mod test {
    use super::*;

    #[test]
    fn test() {
        assert_eq!(func(), 42);
    }
}
