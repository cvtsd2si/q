fn main() -> i32 {
    let a: i16 = -30001;
    let b: i16 = 256;
    let c: i16 = -7;
    let zero: i16 = 0;
    let one: i16 = 1;
    let min: i16 = -32767;
    let max: i16 = 32767;

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
