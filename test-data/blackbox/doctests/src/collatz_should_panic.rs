///
/// ```should_panic
/// use doctests::collatz_should_panic::collatz;
/// #[cfg(not(feature = "changes_should_panic"))]
/// let x = 0;
/// #[cfg(feature = "changes_should_panic")]
/// let x = 1;
/// collatz(x);
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
