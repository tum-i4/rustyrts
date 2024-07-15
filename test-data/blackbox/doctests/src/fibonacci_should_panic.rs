///
/// ```should_panic
/// use doctests::fibonacci_should_panic::fibonacci;
/// let x: u64 = 4;
/// assert_eq!(fibonacci(x), 3);
/// ```
///
pub fn fibonacci(i: u64) -> u64 {
    let mut a = 0;
    let mut b = 1;

    for _ in 0..i {
        let tmp = b;
        b += a;
        a = tmp;
    }

    #[cfg(not(feature = "changes_indirect_should_panic"))]
    panic!();

    a
}
