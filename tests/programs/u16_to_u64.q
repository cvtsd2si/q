fn main() -> i32 {
    let x: u16 = 65_535;
    let y: u64 = (x as u64) + 1;
    print y;
    ret 0;
}
