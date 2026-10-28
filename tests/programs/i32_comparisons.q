fn main() -> i32 {
    let a: i32 = -2147483647;
    let b: i32 = 2147483647;
    let c: i32 = -2147483647;

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
