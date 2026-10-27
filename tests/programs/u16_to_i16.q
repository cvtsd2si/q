fn main() -> i32 {
    let x0: u16 = 65_535;
    let y0: i16 = x0 as i16;
    print y0;

    let x1: u16 = 32_768;
    let y1: i16 = x1 as i16;
    print y1;

    ret 0;
}
