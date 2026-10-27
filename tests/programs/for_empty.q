fn main() -> i32 {
    let x: i32 = 0;
    for (;;) {
        print x;
	if (x >= 4) {
	    break;
	}
	x = x + 1;
    }
    ret 0;
}
