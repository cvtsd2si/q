fn main() -> i32 {
    let x: u64 = 4_294_967_296;
    let y: u8 = x as u8;

    print y;

    let a: u64 = 18_446_744_073_709_551_615;
    let b: u32 = a as u32;

    print b;

    ret 0;
}
