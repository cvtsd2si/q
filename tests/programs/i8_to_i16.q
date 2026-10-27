fn main() -> i32 {
    let x: i8 = 127;
    let y: i16 = (x as i16) + 1;
    print y;

    let a: i8 = -128;
    let b: i16 = a as i16;
    print b;

    ret 0;
}
