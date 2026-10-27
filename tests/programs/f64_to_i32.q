fn main() -> i32 {
    let x0: f64 = 42.75;
    let y0: i32 = x0 as i32;
    print y0;

    let x1: f64 = -42.75;
    let y1: i32 = x1 as i32;
    print y1;

    let x2: f64 = 2147483647.75;
    let y2: i32 = x2 as i32;
    print y2;

    ret 0;
}
