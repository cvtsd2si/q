fn main() -> i32 {
    let a: i8 = -100;
    let b: i8 = 7;
    let c: i8 = -3;
    let zero: i8 = 0;
    let one: i8 = 1;
    let min: i8 = -127;
    let max: i8 = 127;

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
