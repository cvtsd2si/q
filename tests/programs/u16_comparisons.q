fn main() -> i32 {
    let a: u16 = 65535;
    let b: u16 = 0;
    let c: u16 = 65535;

    print (a < b);
    print (a <= b);
    print (a > b);
    print (a >= b);
    print (a == b);
    print (a != b);

    print (a == c);
    print (a != c);

    ret 0;
}
