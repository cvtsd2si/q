fn add_to(ptr: *i64, amount: i64) {
    *ptr = *ptr + amount;
}

fn swap(a: *i64, b: *i64) {
    let temp: i64 = *a;
    *a = *b;
    *b = temp;
}

fn main() -> i64 {
    let arr: [i64; 4] = [10, 20, 30, 40];

    print arr[0];
    print arr[1];
    print arr[2];
    print arr[3];

    let p: *i64 = &arr[1];

    print *p;

    *p = 25;
    print arr[1];

    arr[1] = 26;
    print *p;

    add_to(&arr[2], 5);
    print arr[2];

    let pp: **i64 = &p;

    **pp = 50;
    print arr[1];

    let q: *i64 = &*p;

    *q = 51;
    print arr[1];

    swap(&arr[0], &arr[3]);

    print arr[0];
    print arr[3];

    let second: [i64; 3] = [100, 200, 300];

    let element: *i64 = &second[1];

    *element = 250;

    print second[0];
    print second[1];
    print second[2];

    print arr[1];

    ret 0;
}
