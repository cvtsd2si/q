fn main() -> i64 {
    let x: u16 = 65_535_u16;
    let y: u32 = (x as u32) + 1;
    print y;
    ret 0;
}
