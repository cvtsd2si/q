fn main() -> i32 {
    let x: u8 = 170;
    let y: u8 = 204;

    let and_result: u8 = x & y;
    let or_result: u8 = x | y;
    let xor_result: u8 = x ^ y;

    print and_result;
    print or_result;
    print xor_result;
    ret 0;
}
