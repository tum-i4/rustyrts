struct Foo {
    inner: u32,
}

impl Foo {
    fn new(input: u64) -> Self {
        Foo {
            inner: input as u32,
        }
    }
}

impl Foo {
    fn get(&self) -> u32 {
        self.inner
    }

    fn set(&mut self, input: u32) {
        self.inner = input;
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_static() {
        let foo = Foo::new(42);
    }

    #[test]
    fn test_const() {
        let foo = Foo::new(42);
        assert_eq!(foo.get(), 42);
    }

    #[test]
    fn test_mut() {
        let mut foo = Foo::new(0);
        foo.set(42);
    }
}
