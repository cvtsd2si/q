fn main() -> i32 {
    let a: i32 = -2000000001;
    let b: i32 = 17;
    let zero: i32 = 0;
    let one: i32 = 1;
    let two: i32 = 2;
    let top: i32 = 31;
    let width: i32 = 32;
    let beyond: i32 = 33;
    let large: i32 = 65;
    let max: i32 = 2147483647;
    let min: i32 = -2147483647;
    let c: i32 = -7;

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
    print min << top;
    print c << b;

    ret 0;
}
