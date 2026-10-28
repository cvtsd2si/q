fn main() -> i32 {
    let a: f32 = 7.5;
    let b: f32 = 2.5;
    let c: f32 = -1.25;
    let half: f32 = 0.5;
    let zero: f32 = 0.0;
    let one: f32 = 1.0;

    print a / b;
    print b / c;
    print a / c;
    print c / b;
    print b / half;
    print zero / b;
    print b / b;
    print half / one;
    print a / one;

    ret 0;
}
