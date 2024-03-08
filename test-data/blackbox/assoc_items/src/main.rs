trait CustomTrait<T> {
    #[cfg(not(feature = "changes_assoc_const"))]
    const FOO: i32 = 42;

    #[cfg(feature = "changes_assoc_const")]
    const FOO: i32 = 21;

    type TYPE;

    fn get(self) -> T;

    fn value() -> i32 {
        Self::FOO
    }

    fn ty() -> &'static str {
        std::any::type_name::<Self::TYPE>()
    }
}

impl<'a> CustomTrait<String> for String {
    #[cfg(not(feature = "changes_assoc_type"))]
    type TYPE = i16;

    #[cfg(feature = "changes_assoc_type")]
    type TYPE = f32;

    #[cfg(not(feature = "changes_string"))]
    fn get(self) -> String {
        self
    }

    #[cfg(feature = "changes_string")]
    fn get(self) -> String {
        "".to_string()
    }
}

impl CustomTrait<u32> for i32 {
    type TYPE = f32;

    fn get(self) -> u32 {
        self as u32
    }
}

fn main() {
    println!("Hello, world!");
}

#[test]
fn test_call() {
    let string = "Test".to_string();
    assert_eq!(string.clone().get(), string.clone());

    let signed = 42;
    assert_eq!(signed.get(), signed as u32);
}

#[test]
fn test_assoc_const() {
    assert_eq!(<i32 as CustomTrait<u32>>::value(), 42);
}

#[test]
fn test_assoc_type() {
    assert_eq!(<String as CustomTrait<String>>::ty(), "i16");
}
