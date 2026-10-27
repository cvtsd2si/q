fn main() -> i32 {
    let x0: i16 = -1;
    let y0: u32 = x0 as u32;
    print y0;

    let x1: i16 = -32_768;
    let y1: u32 = x1 as u32;
    print y1;

    ret 0;
}
