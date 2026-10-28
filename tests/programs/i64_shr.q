fn main() -> i32 {
    let a: i64 = -9000000000000000001;
    let b: i64 = 33;
    let zero: i64 = 0;
    let one: i64 = 1;
    let two: i64 = 2;
    let top: i64 = 63;
    let width: i64 = 64;
    let beyond: i64 = 65;
    let large: i64 = 129;
    let max: i64 = 9223372036854775807;
    let min: i64 = -9223372036854775807;
    let c: i64 = -7;

    print a >> zero;
    print a >> one;
    print a >> b;
    print a >> top;
    print a >> width;
    print a >> beyond;
    print a >> large;
    print one >> top;
    print max >> one;
    print zero >> b;
    print b >> b;
    print a >> max;
    print one >> one;
    print a >> two;
    print max >> top;
    print min >> top;
    print c >> b;

    ret 0;
}
