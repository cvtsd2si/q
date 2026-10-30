fn main() {
    goto reachable;
    print 999;

reachable:
    print 7;

    if (false) {
        print 888;
    } else {
        print 8;
    }
    
    ret;

    print 777;
}
