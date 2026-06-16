struct Inner {
    id: Int
}

struct Outer {
    inner: Inner
}

enum Holder {
    Has(Outer)
    Empty
}

fn main() -> Int {
    let inner = Inner { id: 42 }
    let outer = Outer { inner: inner }
    let holder: Holder = Has(outer)
    return 42
}
