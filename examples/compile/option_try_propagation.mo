enum Option<T> {
    Some(T)
    None
}

fn missing() -> Option<Int> {
    return None
}

fn pipeline() -> Option<Int> {
    let value = missing()?
    return Some(value + 100)
}

fn main() -> Int {
    return match pipeline() {
        Some(value) => value
        None => 42
    }
}
