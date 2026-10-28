fn main() -> i32 {
    let a: i32 = -2000000001;
    let b: i32 = 65536;
    let c: i32 = -7;
    let zero: i32 = 0;
    let one: i32 = 1;
    let min: i32 = -2147483647;
    let max: i32 = 2147483647;

    print a / b;
    print b / a;
    print a / c;
    print b / c;
    print c / a;
    print zero / b;
    print max / one;
    print min / one;
    print a / a;
    print max / c;

    ret 0;
}
