#[cfg(not(feature = "changes_static"))]
static CALLBACK: Callback = Callback { func: foo };

#[cfg(feature = "changes_static")]
static CALLBACK: Callback = Callback { func: bar };

static CALLBACK_INDIRECT: Callback = Callback {
    func: CALLBACK.func,
};

struct Callback {
    func: fn() -> i32,
}

#[cfg(not(feature = "changes_fn"))]
fn foo() -> i32 {
    42
}

#[cfg(feature = "changes_fn")]
fn foo() -> i32 {
    41
}

#[allow(dead_code)]
fn bar() -> i32 {
    43
}

fn main() {
    println!("Hello, world!");
}

#[cfg(feature = "test_direct")]
#[test]
fn test_direct() {
    println!("{}", (CALLBACK.func)());
    assert_eq!((CALLBACK.func)(), 42);
}

#[cfg(feature = "test_indirect")]
#[test]
fn test_indirect() {
    println!("{}", (CALLBACK_INDIRECT.func)());
    assert_eq!((CALLBACK_INDIRECT.func)(), 42);
}
