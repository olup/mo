enum Option<T> {
    Some(T)
    None
}

fn main() -> Int {
    let value: Option<Int> = None
    return match value {
        Some(x) => x
        None => 42
    }
}
