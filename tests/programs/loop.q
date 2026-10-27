fn main() -> i64 {
    let x: i64 = 0;
    loop {
        print x;

	x = x + 1;

	if (x == 5) {
	    break;
	}
    }
    ret 0;
}
