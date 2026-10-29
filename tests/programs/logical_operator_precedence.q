fn main() -> i32 {
    print true || false && false;
    print false || true && false;
    print false || true && true;
    print true && false || true; 
    
    print true || false || false && false;
    print false && true || true && true;
    print true && false || false && true;

    ret 0;
}
