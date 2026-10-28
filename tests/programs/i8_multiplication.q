fn main() -> i32 {
    let a: i8 = -12;
    let b: i8 = 7;
    let c: i8 = -5;
    let zero: i8 = 0;
    let one: i8 = 1;
    let min: i8 = -128;
    let max: i8 = 127;

    print a * b;
    print b * a;
    print a * c;
    print b * c;
    print a * zero;
    print min * one;
    print max * one;

    ret 0;
}
