#[test]
pub fn trybuild_test() {
    let t = trybuild::TestCases::new();
    t.pass("../derive/src/main.rs");
}
