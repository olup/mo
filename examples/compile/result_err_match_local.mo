enum Result<T, E> {
    Ok(T)
    Err(E)
}

fn main() -> Int {
    let result: Result<Int, Int> = Err(42)
    let code: Int = match result {
        Ok(value) => value
        Err(error) => error
    }
    return code
}
