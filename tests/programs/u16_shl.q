fn main() -> i32 {
    let a: u16 = 50000;
    let b: u16 = 9;
    let zero: u16 = 0;
    let one: u16 = 1;
    let two: u16 = 2;
    let top: u16 = 15;
    let width: u16 = 16;
    let beyond: u16 = 17;
    let large: u16 = 33;
    let max: u16 = 65535;

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
