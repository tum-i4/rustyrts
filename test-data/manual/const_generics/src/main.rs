struct Foo<const I: usize> {
}

impl<const I: usize>  Foo<I> {
    fn get() -> usize {
        return I;
    }
}

fn main() {
    println!("Hello, world!");
}

#[test]
fn test() {
    assert_eq!(Foo::<9>::get(), 10)
}