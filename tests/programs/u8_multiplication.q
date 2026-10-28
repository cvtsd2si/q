fn main() -> i32 {
    let a: u8 = 15;
    let b: u8 = 17;
    let c: u8 = 7;
    let zero: u8 = 0;
    let one: u8 = 1;
    let max: u8 = 255;

    print a * b;
    print b * a;
    print a * zero;
    print max * one;
    print max * zero;
    print a * one;
    print b * c;

    ret 0;
}
