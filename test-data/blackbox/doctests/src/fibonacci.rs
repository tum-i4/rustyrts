/// ```ignore
/// use doctests::fibonacci::fibonacci;
/// let x = 5;
/// assert_eq!(fibonacci(x), 5);
/// ```
///
/// ```
/// use doctests::fibonacci::fibonacci;
/// let x = 6;
/// assert_eq!(fibonacci(x), 8);
/// ```
///
/// ```no_run
/// use doctests::fibonacci::fibonacci;
/// let x = 3;
/// assert_eq!(fibonacci(x), 2);
/// ```
///
/// ```compile_fail
/// use doctests::fibonacci::fibonacci;
/// let x: i64 = 4;
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

    a
}
