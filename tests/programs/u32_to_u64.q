fn main() -> i32 {
    let x: u32 = 4_294_967_295;
    let y: u64 = (x as u64) + 1;
    print y;
    ret 0;
}
