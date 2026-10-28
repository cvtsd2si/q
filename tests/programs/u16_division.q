fn main() -> i32 {
    let a: u16 = 50000;
    let b: u16 = 128;
    let zero: u16 = 0;
    let one: u16 = 1;
    let two: u16 = 2;
    let three: u16 = 3;
    let max: u16 = 65535;

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
