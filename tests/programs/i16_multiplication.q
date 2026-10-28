fn main() -> i32 {
    let a: i16 = -128;
    let b: i16 = 255;
    let c: i16 = -1;
    let zero: i16 = 0;
    let one: i16 = 1;
    let min: i16 = -32768;
    let max: i16 = 32767;

    print a * b;
    print b * a;
    print a * c;
    print b * c;
    print a * zero;
    print min * one;
    print max * one;

    ret 0;
}
