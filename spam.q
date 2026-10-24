fn main() -> i64 {
    ret add(1, 2);
}

fn add(a: i64, b: i64) -> i64 {
    let x: i64 = 0;
    while (x < 10) {
        print x;
	x = x + 1;
    }
    ret a + b;
}
