fn main() -> i32 {
    let x0: u64 = 18_446_744_073_709_551_615;
    let y0: i64 = x0 as i64;
    print y0;

    let x1: u64 = 9_223_372_036_854_775_808;
    let y1: i64 = x1 as i64;
    print y1;

    ret 0;
}
