fn trace() -> bool {
    print 999;
    ret true;
}

fn main() -> i32 {
    print false && trace();
    print true || trace();
    print true && trace();
    print false || trace();
    
    ret 0;
}
