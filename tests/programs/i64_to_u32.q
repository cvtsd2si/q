fn main() -> i32 {
    let x0: i64 = -1;
    let y0: u32 = x0 as u32;
    print y0;

    let x1: i64 = 4_294_967_296;
    let y1: u32 = x1 as u32;
    print y1;

    let x2: i64 = 2_147_483_648;
    let y2: u32 = x2 as u32;
    print y2;

    ret 0;
}
