fn main() -> i32 {
    let a: u16 = 255;
    let b: u16 = 257;
    let c: u16 = 128;
    let zero: u16 = 0;
    let one: u16 = 1;
    let max: u16 = 65535;

    print a * b;
    print b * a;
    print a * zero;
    print max * one;
    print max * zero;
    print a * one;
    print b * c;

    ret 0;
}
