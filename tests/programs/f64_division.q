fn main() -> i32 {
    let a: f64 = 10.5;
    let b: f64 = 2.0;
    let c: f64 = -0.75;
    let half: f64 = 0.5;
    let zero: f64 = 0.0;
    let precise: f64 = 16777217.0;

    print a / b;
    print a / c;
    print c / b;
    print b / half;
    print zero / b;
    print a / a;
    print precise / b;
    print half / b;
    print a / half;

    ret 0;
}
