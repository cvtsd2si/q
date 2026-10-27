fn main() -> i32 {
    let x: i32 = 65_536;
    let y: i16 = x as i16;
    print y;

    let a: i32 = 65_535;
    let b: i16 = a as i16;
    print b;

    let c: i32 = -32_769;
    let d: i16 = c as i16;
    print d;

    ret 0;
}
