fn main() -> i32 {
    let a: u8 = 200;
    let b: u8 = 7;
    let zero: u8 = 0;
    let one: u8 = 1;
    let two: u8 = 2;
    let three: u8 = 3;
    let max: u8 = 255;

    print a / b;
    print b / a;
    print max / two;
    print max / three;
    print a / one;
    print zero / a;
    print a / a;
    print max / max;
    print one / a;

    ret 0;
}
