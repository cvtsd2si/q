fn main() -> i32 {
    let x: u32 = 2863311530;
    let y: u32 = 3435973836;

    let and_result: u32 = x & y;
    let or_result: u32 = x | y;
    let xor_result: u32 = x ^ y;

    print and_result;
    print or_result;
    print xor_result;
    ret 0;
}
