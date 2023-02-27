struct Foo;

fn bar() -> isize {
    return -1;
}

impl Foo {
    pub fn baz() -> isize {
        // TODO: comment out
        bar()

        // TODO: comment in
        //Self::bar()
    }

    pub fn bar() -> isize {
        return 1;
    }
}

pub fn main() {}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test() {
        assert_eq!(Foo::baz(), Foo::baz());
    }
}
