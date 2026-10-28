fn main() -> i32 {
    let a: i64 = -3037000499;
    let b: i64 = 3037000499;
    let c: i64 = -1;
    let zero: i64 = 0;
    let one: i64 = 1;
    let min: i64 = -9223372036854775807;
    let max: i64 = 9223372036854775807;

    print a * b;
    print b * a;
    print a * c;
    print b * c;
    print a * zero;
    print min * one;
    print max * one;

    ret 0;
}
