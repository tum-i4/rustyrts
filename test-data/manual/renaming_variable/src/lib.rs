#![feature(core_intrinsics)]

use std::intrinsics::size_of;

static VAR_STATIC: i32 = 42;
const VAR_CONST: i32 = init();

const fn init() -> i32 {
    42
}

fn reference() -> i32 {
    42
}

const fn fib(input: usize) -> usize {
    if input == 0 || input == 1 {
        return 1;
    }
    fib(input - 1) + fib(input - 1)
}

struct Foo<const size: usize> {
    data: [i32; size],
}

impl<const size: usize> Foo<size> {
    fn new() -> Self {
        Foo { data: [0; size] }
    }

    fn set(&mut self, input: i32) {
        self.data[0] = 1;
    }

    fn get(&self) -> i32 {
        self.data[0]
    }
}

#[test]
fn test() {
    assert_eq!(VAR_STATIC, reference());
    assert_eq!(VAR_CONST, reference());
}

#[test]
fn test_const_generic() {
    let mut foo: Foo<{ fib(5) }> = Foo::new();
    foo.set(1);
    assert_eq!(foo.get(), 1);

    assert_eq!(size_of::<Foo<{ fib(5) }>>(), 32);
}
