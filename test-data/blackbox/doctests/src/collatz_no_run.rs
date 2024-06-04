///
/// ```no_run
/// use doctests::collatz_no_run::collatz;
/// #[cfg(not(feature = "changes_no_run"))]
/// let x: u64 = 13;
/// #[cfg(feature = "changes_no_run")]
/// let x: i64 = 12;
/// assert_eq!(collatz(x), 40);
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
