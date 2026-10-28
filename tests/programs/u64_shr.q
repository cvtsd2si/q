fn main() -> i32 {
    let a: u64 = 18000000000000000000;
    let b: u64 = 33;
    let zero: u64 = 0;
    let one: u64 = 1;
    let two: u64 = 2;
    let top: u64 = 63;
    let width: u64 = 64;
    let beyond: u64 = 65;
    let large: u64 = 129;
    let max: u64 = 18446744073709551615;

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
    print max >> max;
    print zero >> max;

    ret 0;
}
