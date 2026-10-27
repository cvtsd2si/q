fn main() -> i32 {
    let x: i16 = 32_767;
    let y: i32 = (x as i32) + 1;
    print y;

    let a: i16 = -32_768;
    let b: i32 = a as i32;
    print b;

    ret 0;
}
