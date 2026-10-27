fn main() -> i32 {
    let x0: f32 = 42.75;
    let y0: i32 = x0 as i32;
    print y0;

    let x1: f32 = -42.75;
    let y1: i32 = x1 as i32;
    print y1;

    let x2: f32 = 1073741824.0;
    let y2: i32 = x2 as i32;
    print y2;

    ret 0;
}
