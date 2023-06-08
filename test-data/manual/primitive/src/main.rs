trait CustomTrait<T> {
    const FOO: i32 = 42;

    fn get(self) -> T;

    fn value() -> i32 {
        Self::FOO
    }
}

impl<'a> CustomTrait<&'a String> for &'a String {
    fn get(self) -> &'a String {
        println!("Test");
        &self
    }

    fn value() -> i32 {
        21
    }
}

impl<'a> CustomTrait<&'a str> for &'a str {
    fn get(self) -> &'a str {
        &self
    }
}

impl CustomTrait<u32> for i32 {
    fn get(self) -> u32 {
        println!("Test");
        self as u32
    }
}

fn main() {
    println!("Hello, world!");
}

#[test]
fn test_primitive() {
    let string = "Test".to_string();
    assert_eq!(*string.get(), string);

    let str_slice = &string;
    assert_eq!(*str_slice.get(), string);

    let signed = 42;
    assert_eq!(signed.get(), signed as u32);
}

#[test]
fn test_assoc_const() {
    assert_eq!(<i32 as CustomTrait<u32>>::value(), 42);
}
