fn main() -> i32 {
    let x: u64 = 12297829382473034410;
    let y: u64 = 14757395258967641292;

    let and_result: u64 = x & y;
    let or_result: u64 = x | y;
    let xor_result: u64 = x ^ y;

    print and_result;
    print or_result;
    print xor_result;
    ret 0;
}
