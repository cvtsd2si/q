fn choose(flag: bool, left: i32, right: i32) -> i32 {
    let result: i32 = 0;
    if (flag) {
        result = left;
    } else {
        result = right;
    }
    ret result;
}

fn main() {
    print choose(true, 11, 22);
    print choose(false, 11, 22);
}
