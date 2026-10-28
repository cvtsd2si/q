fn main() -> i32 {
    let a: u32 = 0;
    let b: u32 = 4294967295;
    let c: u32 = 0;

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
