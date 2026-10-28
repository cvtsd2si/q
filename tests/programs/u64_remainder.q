fn main() -> i32 {
    let a: u64 = 18000000000000000000;
    let b: u64 = 4294967296;
    let zero: u64 = 0;
    let one: u64 = 1;
    let two: u64 = 2;
    let three: u64 = 3;
    let max: u64 = 18446744073709551615;

    print a % b;
    print b % a;
    print max % two;
    print max % three;
    print a % one;
    print zero % a;
    print a % a;
    print max % max;
    print one % a;

    ret 0;
}
