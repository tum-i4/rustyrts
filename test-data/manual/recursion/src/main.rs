fn main() {
    println!("Hello, world!");
}

// The purpose of this crate is, to verify that recursive methods do only occur once in the traces of dynamic RustyRTS

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
