fn widen(input: i32) -> i64 {
    let widened: i64 = input as i64;
    let copied: i64 = widened;
    ret copied;
}

fn main() {
    print widen(42);
}
