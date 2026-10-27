fn main() -> i32 {
    let x0: u16 = 65_535;
    let y0: i64 = x0 as i64;
    print y0;

    let x1: u16 = 32_768;
    let y1: i64 = x1 as i64;
    print y1;

    ret 0;
}
