fn main() -> i32 {
    let x: i32 = -1431655766;
    let y: i32 = -858993460;

    let and_result: i32 = x & y;
    let or_result: i32 = x | y;
    let xor_result: i32 = x ^ y;

    print and_result;
    print or_result;
    print xor_result;
    ret 0;
}
