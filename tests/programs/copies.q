fn chain(input: i32) -> i32 {
    let a: i32 = input;
    let b: i32 = a;
    let c: i32 = b;
    ret c;
}

fn main() {
    print chain(17);
}
