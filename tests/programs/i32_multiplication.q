fn main() -> i32 {
    let a: i32 = -46340;
    let b: i32 = 46340;
    let c: i32 = -1;
    let zero: i32 = 0;
    let one: i32 = 1;
    let min: i32 = -2147483647;
    let max: i32 = 2147483647;

    print a * b;
    print b * a;
    print a * c;
    print b * c;
    print a * zero;
    print min * one;
    print max * one;

    ret 0;
}
