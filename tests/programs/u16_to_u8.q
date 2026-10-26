fn main() -> i32 {
    let x: u16 = 256;
    let y: u8 = x as u8;
    
    print y;

    let a: u16 = 511;
    let b: u8 = a as u8;

    print b;

    ret 0;
}
