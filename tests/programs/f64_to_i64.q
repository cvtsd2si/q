fn main() -> i32 {
    let x0: f64 = 42.75;
    let y0: i64 = x0 as i64;
    print y0;

    let x1: f64 = -42.75;
    let y1: i64 = x1 as i64;
    print y1;

    let x2: f64 = 9007199254740992.0;
    let y2: i64 = x2 as i64;
    print y2;

    ret 0;
}
