fn func<F>(fun: F, input: u64) -> u64
where
    F: Fn(u64) -> u64,
{
    fun(input)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test() {
        assert_eq!(func(|x| x, 42), 42);
    }
}
