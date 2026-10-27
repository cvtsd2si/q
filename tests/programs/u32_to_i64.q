fn main() -> i32 {
    let x0: u32 = 4_294_967_295;
    let y0: i64 = x0 as i64;
    print y0;

    let x1: u32 = 2_147_483_648;
    let y1: i64 = x1 as i64;
    print y1;

    ret 0;
}
