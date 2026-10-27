fn main() -> i32 {
    let x: i8 = 127;
    let y: i64 = (x as i64) + 1;
    print y;

    let a: i8 = -128;
    let b: i64 = a as i64;
    print b;

    ret 0;
}
