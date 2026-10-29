fn main() -> i32 {
    let x: u16 = 43690;
    let y: u16 = 52428;

    let and_result: u16 = x & y;
    let or_result: u16 = x | y;
    let xor_result: u16 = x ^ y;

    print and_result;
    print or_result;
    print xor_result;
    ret 0;
}
