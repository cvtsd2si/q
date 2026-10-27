fn main() -> i32 {
    let x0: i32 = -1;
    let y0: u64 = x0 as u64;
    print y0;

    let x1: i32 = -2_147_483_648;
    let y1: u64 = x1 as u64;
    print y1;

    ret 0;
}
