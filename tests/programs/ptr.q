fn increment(ptr: *i64) {
    *ptr = *ptr + 1;
}

fn set_value(ptr: *i64, value: i64) {
    *ptr = value;
}

fn increment_double(ptr: **i64) {
    **ptr = **ptr + 1;
}

fn main() -> i64 {
    let x: i64 = 10;

    let p: *i64 = &x;
    print *p;

    *p = 20;
    print x;

    x = 30;
    print *p;

    increment(&x);
    print x;

    set_value(p, 50);
    print x;

    let pp: **i64 = &p;
    print **pp;

    **pp = 60;
    print x;

    increment_double(&p);
    print x;

    let q: *i64 = &*p;
    *q = 70;
    print x;

    let y: i64 = 100;
    *pp = &y;

    print *p;

    *p = 200;

    print x;
    print y;

    ret 0;
}
