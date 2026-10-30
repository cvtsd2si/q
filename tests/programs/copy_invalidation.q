fn check(input: i32) -> i32 {
    let a: i32 = input;
    let b: i32 = a;
    a = 99;
    print b;
    ret a;
}

fn main() {
    print check(7);
}
