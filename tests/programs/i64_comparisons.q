fn main() -> i32 {
    let a: i64 = 9223372036854775807;
    let b: i64 = -9223372036854775807;
    let c: i64 = 9223372036854775807;

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
