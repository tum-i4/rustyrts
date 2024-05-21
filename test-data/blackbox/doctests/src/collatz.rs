/// ```ignore
/// use doctests::collatz::collatz;
/// let x = 11;
/// assert_eq!(collatz(x), 34);
/// ```
///
/// ```
/// use doctests::collatz::collatz;
/// let x = 12;
/// assert_eq!(collatz(x), 6);
/// ```
///
/// ```no_run
/// use doctests::collatz::collatz;
/// let x = 13;
/// assert_eq!(collatz(x), 40);
/// ```
///
/// ```compile_fail
/// use doctests::collatz::collatz;
/// let x: i64 = 12;
/// assert_eq!(collatz(x), 6);
/// ```
///
/// ```should_panic
/// use doctests::collatz::collatz;
/// let x = 0;
/// assert_eq!(collatz(x), 0);
/// ```
///
pub fn collatz(i: u64) -> u64 {
    if i == 0 {
        panic!("")
    }

    if i % 2 == 0 {
        i / 2
    } else {
        3 * i + 1
    }
}
