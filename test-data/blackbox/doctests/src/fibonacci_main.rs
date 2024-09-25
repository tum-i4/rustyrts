///
/// ```
/// use doctests::fibonacci_main::fibonacci;
///
/// fn main() {
///     let x = 6;
///     assert_eq!(fibonacci(x), 8);
/// }
/// ```
///
pub fn fibonacci(i: u64) -> u64 {
    let mut a = 0;
    let mut b = 1;

    #[cfg(feature = "changes_indirect_main")]
    return u64::MAX;

    for _ in 0..i {
        let tmp = b;
        b += a;
        a = tmp;
    }

    a
}
