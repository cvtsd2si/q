fn main() -> i32 {
    let x: u32 = 65_536;
    let y: u8 = x as u8;
    
    print y;

    let a: u32 = 4_294_967_295;
    let b: u8 = a as u8;

    print b;

    ret 0;
}
