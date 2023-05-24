static VAR_STATIC: i32 = 42;
const VAR_CONST: i32 = init();

const fn init() -> i32 {
    42
}

#[test]
fn test() {
    assert_eq!(VAR_STATIC, 42);
    assert_eq!(VAR_CONST, 42);
}
