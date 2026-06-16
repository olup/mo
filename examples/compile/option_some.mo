enum Option<T> {
    Some(T)
    None
}

fn main() -> Int {
    let value: Option<Int> = Some(42)
    return match value {
        Some(x) => x
        None => 0
    }
}
