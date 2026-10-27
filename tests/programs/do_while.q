fn main() -> i32 {
    let x: i32 = 0;
    do {
        print x;
	x = x + 1;
    } while (x < 5);
    ret 0;
}
