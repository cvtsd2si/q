fn main() -> i32 {
    let x0: u64 = 65_536;
    let y0: i16 = x0 as i16;
    print y0;

    let x1: u64 = 65_535;
    let y1: i16 = x1 as i16;
    print y1;

    let x2: u64 = 32_768;
    let y2: i16 = x2 as i16;
    print y2;

    ret 0;
}
