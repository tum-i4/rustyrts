/// ```
/// use doctests::collatz::collatz;
/// #[cfg(not(feature = "changes_run"))]
/// let x = 12;
/// #[cfg(feature = "changes_run")]
/// let x = 11;
/// assert_eq!(collatz(x), 6);
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
