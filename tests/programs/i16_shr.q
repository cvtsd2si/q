fn main() -> i32 {
    let a: i16 = -30001;
    let b: i16 = 9;
    let zero: i16 = 0;
    let one: i16 = 1;
    let two: i16 = 2;
    let top: i16 = 15;
    let width: i16 = 16;
    let beyond: i16 = 17;
    let large: i16 = 33;
    let max: i16 = 32767;
    let min: i16 = -32767;
    let c: i16 = -7;

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
