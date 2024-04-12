static mut DROPPED: bool = false;

struct Foo {
    data: i32,
}
struct Bar {
    data: i32,
    foo: Foo,
}

#[cfg(feature = "drop_inner")]
impl Drop for Foo {
    fn drop(&mut self) {
        println!("Data {}", self.data);
        #[cfg(not(feature = "changes_drop"))]
        unsafe {
            DROPPED = true;
        }
    }
}

#[cfg(feature = "drop_outer")]
impl Drop for Bar {
    fn drop(&mut self) {
        println!("Data {}", self.data);
        println!("Foo {}", self.foo.data);
        #[cfg(not(feature = "changes_drop"))]
        unsafe {
            DROPPED = true;
        }
    }
}

#[cfg(feature = "drop_delegate")]
fn delegate<T>(t: T) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_drop() {
        {
            let bar = Bar {
                data: 1,
                foo: Foo { data: 2 },
            };

            #[cfg(feature = "drop_direct")]
            drop(bar);

            #[cfg(feature = "drop_delegate")]
            delegate(bar);

            #[cfg(feature = "drop_closure")]
            {
                let f = move || {
                    let b = bar;
                };
                f();
            }
        }
        assert!(unsafe { DROPPED })
    }
}
