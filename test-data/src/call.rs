fn func() -> u64 {
    42
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test() {
        assert_eq!(func(), 42);
    }
}
