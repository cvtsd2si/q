fn main() -> i32 {
    let a: u32 = 65535;
    let b: u32 = 65537;
    let c: u32 = 32768;
    let zero: u32 = 0;
    let one: u32 = 1;
    let max: u32 = 4294967295;

    print a * b;
    print b * a;
    print a * zero;
    print max * one;
    print max * zero;
    print a * one;
    print b * c;

    ret 0;
}
