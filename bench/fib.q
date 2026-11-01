fn main() -> u64 {
    print fib(40);
    ret 0;
}

fn fib(n: u64) -> u64 {
   if (n < 2) ret n;
   ret fib(n-1) + fib(n-2);
}
