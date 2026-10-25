fn main() -> i64 {
    ret add(1, 2);
}

fn add(a: i64, b: i64) -> i64 {
    let x: i64 = 0;
    for (;;) {
        print x;
	x = x + 1;
	if (x == 10) {
	    break;
	}
    }
    ret a + b;
}
