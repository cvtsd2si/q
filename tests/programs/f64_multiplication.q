fn main() -> i32 {
    let a: f64 = 1.25;
    let b: f64 = 2.5;
    let c: f64 = -1.5;
    let half: f64 = 0.5;
    let precise: f64 = 16777217.0;

    print a * b;
    print b * a;
    print a * c;
    print b * c;
    print c * c;
    print a * half;
    print precise * half;

    ret 0;
}
