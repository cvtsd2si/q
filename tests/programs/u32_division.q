fn main() -> i32 {
    let a: u32 = 4000000000;
    let b: u32 = 65536;
    let zero: u32 = 0;
    let one: u32 = 1;
    let two: u32 = 2;
    let three: u32 = 3;
    let max: u32 = 4294967295;

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
