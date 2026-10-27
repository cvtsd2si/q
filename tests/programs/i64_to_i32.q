fn main() -> i32 {
    let x: i64 = 4_294_967_296;
    let y: i32 = x as i32;
    print y;

    let a: i64 = 4_294_967_295;
    let b: i32 = a as i32;
    print b;

    let c: i64 = -2_147_483_649;
    let d: i32 = c as i32;
    print d;

    ret 0;
}
