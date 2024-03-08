fn main() {
    println!("Hello, world!");
}

#[test]
fn test1() {
    let map1 = |input| input + 1;

    let ints = Vec::from(&[1, 2, 3, 4]);
    let actual: Vec<i32> = ints.iter().map(map1).collect();
    let expected = Vec::from(&[2, 3, 4, 5]);

    assert_eq!(actual, expected);
}

#[test]
fn test2() {
    let map2 = |input| input * input;

    let ints = Vec::from(&[1, 2, 3, 4]);
    let actual: Vec<i32> = ints.iter().map(map2).collect();
    let expected = Vec::from(&[1, 4, 9, 16]);

    assert_eq!(actual, expected);
}
