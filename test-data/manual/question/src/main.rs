use std::num::ParseIntError;

fn main() -> Result<(), ParseIntError> {
    let u32_str = "42";
    let uint: u32 = u32_str.parse()?;

    let i32_str = "-32";
    let int: i32 = i32_str.parse()?;

    println!("{} and {}", uint, int);

    return Ok(());
}

#[test]
fn test_main() {
    main().unwrap();
}
