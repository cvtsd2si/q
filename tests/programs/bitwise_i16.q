fn main() -> i32 {
    let x: i16 = -21846;
    let y: i16 = -13108;

    let and_result: i16 = x & y;
    let or_result: i16 = x | y;
    let xor_result: i16 = x ^ y;

    print and_result;
    print or_result;
    print xor_result;
    ret 0;
}
