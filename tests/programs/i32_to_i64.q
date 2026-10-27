fn main() -> i32 {
    let x: i32 = 2_147_483_647;
    let y: i64 = (x as i64) + 1;
    print y;

    let a: i32 = -2_147_483_648;
    let b: i64 = a as i64;
    print b;

    ret 0;
}
