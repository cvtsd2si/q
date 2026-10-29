fn main() -> i32 {
    let x: i64 = -6148914691236517206;
    let y: i64 = -3689348814741910324;

    let and_result: i64 = x & y;
    let or_result: i64 = x | y;
    let xor_result: i64 = x ^ y;

    print and_result;
    print or_result;
    print xor_result;
    ret 0;
}
