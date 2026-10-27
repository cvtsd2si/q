fn main() -> i32 {
    let x: i16 = 32_767;
    let y: i64 = (x as i64) + 1;
    print y;

    let a: i16 = -32_768;
    let b: i64 = a as i64;
    print b;

    ret 0;
}
