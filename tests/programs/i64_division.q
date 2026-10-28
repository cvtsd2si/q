fn main() -> i32 {
    let a: i64 = -9000000000000000001;
    let b: i64 = 4294967296;
    let c: i64 = -7;
    let zero: i64 = 0;
    let one: i64 = 1;
    let min: i64 = -9223372036854775807;
    let max: i64 = 9223372036854775807;

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
