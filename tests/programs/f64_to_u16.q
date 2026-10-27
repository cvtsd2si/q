fn main() -> i32 {
    let x0: f64 = 42.75;
    let y0: u16 = x0 as u16;
    print y0;

    let x1: f64 = 0.75;
    let y1: u16 = x1 as u16;
    print y1;

    let x2: f64 = 65535.75;
    let y2: u16 = x2 as u16;
    print y2;

    ret 0;
}
