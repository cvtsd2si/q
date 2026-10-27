fn main() -> i32 {
    let x0: u64 = 42;
    let y0: f64 = x0 as f64;
    print y0;

    let x1: u64 = 9_223_372_036_854_775_808;
    let y1: f64 = x1 as f64;
    print y1;

    ret 0;
}
