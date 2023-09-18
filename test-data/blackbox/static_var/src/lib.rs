pub const fn add(left: usize, right: usize) -> usize {
    left + right
}

#[cfg(not(feature = "changes_immutable"))]
static IMMUTABLE: usize = add(1, 2);

#[cfg(feature = "changes_immutable")]
static IMMUTABLE: usize = add(2, 2);

#[cfg(not(feature = "changes_mutable"))]
static mut MUTABLE: usize = add(3, 4);

#[cfg(feature = "changes_mutable")]
static mut MUTABLE: usize = add(4, 4);

#[test]
fn test_immutable() {
    assert_eq!(IMMUTABLE, 3);
}

#[test]
fn test_mutable() {
    assert_eq!(unsafe { MUTABLE }, 7);
    unsafe { MUTABLE = 42 };
    assert_eq!(unsafe { MUTABLE }, 42);
}
