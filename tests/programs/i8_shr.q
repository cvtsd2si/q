fn main() -> i32 {
    let a: i8 = -100;
    let b: i8 = 3;
    let zero: i8 = 0;
    let one: i8 = 1;
    let two: i8 = 2;
    let top: i8 = 7;
    let width: i8 = 8;
    let beyond: i8 = 9;
    let large: i8 = 17;
    let max: i8 = 127;
    let min: i8 = -127;
    let c: i8 = -3;

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
