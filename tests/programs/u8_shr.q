fn main() -> i32 {
    let a: u8 = 200;
    let b: u8 = 3;
    let zero: u8 = 0;
    let one: u8 = 1;
    let two: u8 = 2;
    let top: u8 = 7;
    let width: u8 = 8;
    let beyond: u8 = 9;
    let large: u8 = 17;
    let max: u8 = 255;

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
