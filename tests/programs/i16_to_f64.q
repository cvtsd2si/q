fn main() -> i32 {
    let x0: i16 = -42;
    let y0: f64 = x0 as f64;
    print y0;

    let x1: i16 = 32_767;
    let y1: f64 = x1 as f64;
    print y1;

    ret 0;
}
