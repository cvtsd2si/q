fn main() -> i32 {
    let x0: f32 = 42.75;
    let y0: u32 = x0 as u32;
    print y0;

    let x1: f32 = 0.75;
    let y1: u32 = x1 as u32;
    print y1;

    let x2: f32 = 2147483648.0;
    let y2: u32 = x2 as u32;
    print y2;

    ret 0;
}
