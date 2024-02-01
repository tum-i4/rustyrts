use std::fmt::Display;

struct Foo<T> where T: Display{
    data1: T,
    data2: T
}

impl<T> Foo<T> where T: Display{
    
    #[cfg(not(feature = "changes_unused"))]
    fn data(&self) -> &T {
        &self.data1
    }

    #[cfg(feature = "changes_unused")]
    fn data(&self) -> &T {
        &self.data2
    }
}

// Not sure if this is a bug...
// The 'unused lifetime here is not referred to anywhere else
// Still, it is part of the generic args of the drop function
//
// This test checks if rustyrts incorporates the lifetime properly
// If not, rustyrts crashes

impl<'unused, T> Drop for Foo<T> where T: Display{
    fn drop(&mut self) {
        println!("Dropped: {}", self.data());
    }
}


#[test]
fn test() {
    let foo = Foo{data1: 1, data2: 2};
    assert_eq!(*foo.data(), 1);
}

fn main() {
    
}
