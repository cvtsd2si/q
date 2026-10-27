fn main() -> i32 {
    let x0: i64 = -1;
    let y0: u64 = x0 as u64;
    print y0;

    let x1: i64 = -9_223_372_036_854_775_808;
    let y1: u64 = x1 as u64;
    print y1;

    ret 0;
}
