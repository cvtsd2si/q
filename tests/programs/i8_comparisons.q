fn main() -> i32 {
    let a: i8 = -128;
    let b: i8 = 127;
    let c: i8 = -128;

    print a < b;
    print a <= b;
    print a > b;
    print a >= b;
    print a == b;
    print a != b;

    print a == c;
    print a != c;

    ret 0;
}
