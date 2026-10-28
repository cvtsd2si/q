fn main() -> i32 {
    let a: u64 = 18446744073709551615;
    let b: u64 = 0;
    let c: u64 = 18446744073709551615;

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
