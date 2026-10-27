fn main() -> i32 {
    let x0: f64 = 42.75;
    let y0: u32 = x0 as u32;
    print y0;

    let x1: f64 = 0.75;
    let y1: u32 = x1 as u32;
    print y1;

    let x2: f64 = 4294967295.75;
    let y2: u32 = x2 as u32;
    print y2;

    ret 0;
}
