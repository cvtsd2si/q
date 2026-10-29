fn noisy() -> i32 {
    print 77;
    ret 1;
}

fn main() {
    let unused: i32 = noisy();
    print 5;
}
