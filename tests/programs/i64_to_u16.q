fn main() -> i32 {
    let x0: i64 = -1;
    let y0: u16 = x0 as u16;
    print y0;

    let x1: i64 = 65_536;
    let y1: u16 = x1 as u16;
    print y1;

    let x2: i64 = 32_768;
    let y2: u16 = x2 as u16;
    print y2;

    ret 0;
}
