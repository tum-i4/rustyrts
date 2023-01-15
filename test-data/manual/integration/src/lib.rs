pub fn involved(input: usize) -> usize {
    return input + 1;
}

pub fn uninvolved(input: usize) -> usize {
    return input - 1;
}

#[test]
pub fn test_uninvolved() {
    assert_eq!(2, uninvolved(3))
}
