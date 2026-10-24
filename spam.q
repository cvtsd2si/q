fn main() -> i64 {
   let n: i64 = fib(40);
   print n;
   ret 0;
}

fn fib(n: i64) -> i64 {
   if (n < 2) {
       ret n;
   }
   ret fib(n-1) + fib(n-2);
}
