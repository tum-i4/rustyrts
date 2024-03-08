trait ATrait {
    fn fun_a(&self) -> i32;
}

trait BTrait {
    fn fun_b(self) -> i32;
}

trait CTrait {
    fn fun_c(self) -> i32;
}

impl<T> ATrait for T
where
    for<'a> &'a T: CTrait,
{
    fn fun_a(&self) -> i32 {
        BTrait::fun_b(self)
    }
}

impl<T> BTrait for &T
where
    for<'a> &'a T: CTrait,
{
    fn fun_b(self) -> i32 {
        CTrait::fun_c(self)
    }
}

impl CTrait for &AStruct {
    fn fun_c(self) -> i32 {
        fun()
    }
}

#[cfg(not(feature = "changes_inner"))]
fn fun() -> i32 {
    42
}
#[cfg(feature = "changes_inner")]
#[inline(never)]
fn fun() -> i32 {
    41
}

struct AStruct();

#[test]
fn test() {
    let s = AStruct {};
    let d: &dyn ATrait = &s;
    assert_eq!(d.fun_a(), 42);
}

#[test]
fn another_test() {
    let s = AStruct {};
    assert_eq!(s.fun_a(), 42);
}

fn main() {
    println!("Hello, world!");
}
