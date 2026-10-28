fn main() -> i32 {
    let a: i16 = 32767;
    let b: i16 = -32768;
    let c: i16 = 32767;

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
