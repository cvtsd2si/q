fn main() -> i32 {
    let x0: f64 = 42.75;
    let y0: i16 = x0 as i16;
    print y0;

    let x1: f64 = -42.75;
    let y1: i16 = x1 as i16;
    print y1;

    let x2: f64 = 32767.75;
    let y2: i16 = x2 as i16;
    print y2;

    ret 0;
}
