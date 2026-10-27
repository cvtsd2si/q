fn main() -> i32 {
    let x: i8 = 127;
    let y: i32 = (x as i32) + 1;
    print y;

    let a: i8 = -128;
    let b: i32 = a as i32;
    print b;

    ret 0;
}
