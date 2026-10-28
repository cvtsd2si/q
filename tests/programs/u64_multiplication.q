fn main() -> i32 {
    let a: u64 = 4294967295;
    let b: u64 = 4294967297;
    let c: u64 = 65536;
    let zero: u64 = 0;
    let one: u64 = 1;
    let max: u64 = 18446744073709551615;

    print a * b;
    print b * a;
    print a * zero;
    print max * one;
    print max * zero;
    print a * one;
    print b * c;

    ret 0;
}
