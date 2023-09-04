fn main() {}

fn foo(subject: &mut bool, f: &mut dyn FnMut(&mut bool)) {
    f(subject);
}

#[test]
fn test_dyn_fn() {
    let mut b = false;

    foo(&mut b, &mut |s: &mut bool| {
        *s = false;
    });

    assert!(b);
}
