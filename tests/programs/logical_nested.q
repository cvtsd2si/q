fn main() -> i32 {
    print true && true && true; 
    print true && true && false;
    print false && true && true;
    
    print false || false || true;
    print false || false || false;
    print true || false || false;
    
    print (true && false) || true;
    print (true || false) && false;
    print (true && true) || false;
    print (false || false) && true;
    
    print (true || false) && (true || false);
    print (true && false) || (false && true);
    print (false || true) && (true && true);
    
    ret 0;
}
