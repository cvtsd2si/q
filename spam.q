fn add(a: i64, b: i64) -> i64 {
    ret a + b;
}

fn sub(a: i64, b: i64) -> i64 {
    ret a - b;
}

fn main() -> i64 {
   let x: i64 = 1;
   let y: i64 = 2;
   ret add(x, y) + sub(y, x);
}
