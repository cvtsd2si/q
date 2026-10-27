fn main() -> i32 {
    let x0: f32 = 42.75;
    let y0: u64 = x0 as u64;
    print y0;

    let x1: f32 = 0.75;
    let y1: u64 = x1 as u64;
    print y1;

    let x2: f32 = 9223372036854775808.0;
    let y2: u64 = x2 as u64;
    print y2;

    ret 0;
}
