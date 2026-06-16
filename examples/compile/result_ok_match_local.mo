enum Result<T, E> {
    Ok(T)
    Err(E)
}

fn main() -> Int {
    let result: Result<Int, Int> = Ok(41)
    let code: Int = match result {
        Ok(value) => value + 1
        Err(error) => error
    }
    return code
}
