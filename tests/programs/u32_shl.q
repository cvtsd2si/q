fn main() -> i32 {
    let a: u32 = 4000000000;
    let b: u32 = 17;
    let zero: u32 = 0;
    let one: u32 = 1;
    let two: u32 = 2;
    let top: u32 = 31;
    let width: u32 = 32;
    let beyond: u32 = 33;
    let large: u32 = 65;
    let max: u32 = 4294967295;

    print a << zero;
    print a << one;
    print a << b;
    print a << top;
    print a << width;
    print a << beyond;
    print a << large;
    print one << top;
    print max << one;
    print zero << b;
    print b << b;
    print a << max;
    print one << one;
    print a << two;
    print max << top;
    print max << max;
    print zero << max;

    ret 0;
}
