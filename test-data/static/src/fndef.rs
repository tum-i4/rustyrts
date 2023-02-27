fn incr(input: u64) -> u64 {
    input + 1
}

fn apply<F>(func: F, input: u64) -> u64
where
    F: Fn(u64) -> u64,
{
    func(input)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_indirect() {
        let x = incr;
        assert_eq!(x(41), 42);
    }

    #[test]
    fn test_higher_order() {
        assert_eq!(apply(incr, 41), 42);
    }
}
