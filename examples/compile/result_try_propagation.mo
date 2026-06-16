enum Result<T, E> {
    Ok(T)
    Err(E)
}

fn fail() -> Result<Int, Int> {
    return Err(7)
}

fn pipeline() -> Result<Int, Int> {
    let value = fail()?
    return Ok(value + 100)
}

fn main() -> Int {
    return match pipeline() {
        Ok(value) => value
        Err(error) => {
            if error == 7 {
                return 42
            }
            return 1
        }
    }
}
