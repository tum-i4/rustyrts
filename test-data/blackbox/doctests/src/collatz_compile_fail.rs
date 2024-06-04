///
/// ```compile_fail
/// use doctests::collatz_compile_fail::collatz;
/// #[cfg(not(feature = "changes_compile_fail"))]
/// let x: i64 = 12;
/// #[cfg(feature = "changes_compile_fail")]
/// let x: u64 = 12;
/// assert_eq!(collatz(x), 6);
/// ```
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
