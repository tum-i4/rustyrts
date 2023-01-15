fn main() {
    println!("Hello, world!");
}

fn recursive(input: usize, count: usize) -> usize {
    if input == 1 {
        return count;
    }
    recursive(input / 2, count + 1)
}

#[test]
pub fn test() {
    print!("Test");
    assert_eq!(4, recursive(16, 0))
}
