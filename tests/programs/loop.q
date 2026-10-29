fn main() {
    let i: i32 = 0;
    let sum: i32 = 0;
    while (i < 5) {
        let snapshot: i32 = i;
        sum = sum + snapshot;
        i = i + 1;
    }
    print sum;
}
