fn main() -> i32 {
    let x0: u16 = 0;
    let y0: f32 = x0 as f32;
    print y0;

    let x1: u16 = 65_535;
    let y1: f32 = x1 as f32;
    print y1;

    ret 0;
}
