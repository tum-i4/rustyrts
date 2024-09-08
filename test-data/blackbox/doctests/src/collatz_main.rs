/// ```
/// use doctests::collatz::collatz;
///
/// fn main() {
///     #[cfg(not(feature = "changes_main"))]
///     let x = 12;
///     #[cfg(feature = "changes_main")]
///     let x = 11;
///     assert_eq!(collatz(x), 6);
/// }
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
