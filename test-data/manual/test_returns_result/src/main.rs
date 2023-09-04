fn main() {
    println!("Hello, world!");
}

fn foo() -> bool {
    return 1 == 1;
}

#[test]
fn test_normal() {
    assert!(foo())
}

#[test]
fn test_result() -> Result<(), ()> {
    if foo() {
        Ok(())
    } else {
        Err(())
    }
}

#[tokio::test]
async fn test_tokio() -> Result<(), ()> {
    if foo() {
        Ok(())
    } else {
        Err(())
    }
}
