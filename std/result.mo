pub enum Result<T, E> {
    Ok(T)
    Err(E)
}

pub fn is_ok<T, E>(value: Result<T, E>) -> Bool {
    return match value {
        Ok(item) => true
        Err(error) => false
    }
}

pub fn is_err<T, E>(value: Result<T, E>) -> Bool {
    return match value {
        Ok(item) => false
        Err(error) => true
    }
}

pub fn unwrap_or<T, E>(value: Result<T, E>, fallback: T) -> T {
    return match value {
        Ok(item) => item
        Err(error) => fallback
    }
}

pub fn map<T, E, U>(value: Result<T, E>, mapper: fn(T) -> U) -> Result<U, E> {
    return match value {
        Ok(item) => Ok(mapper(item))
        Err(error) => Err(error)
    }
}

pub fn and_then<T, E, U>(value: Result<T, E>, mapper: fn(T) -> Result<U, E>) -> Result<U, E> {
    return match value {
        Ok(item) => mapper(item)
        Err(error) => Err(error)
    }
}

pub fn map_err<T, E, F>(value: Result<T, E>, mapper: fn(E) -> F) -> Result<T, F> {
    return match value {
        Ok(item) => Ok(item)
        Err(error) => Err(mapper(error))
    }
}

pub fn or_else<T, E, F>(value: Result<T, E>, fallback: fn(E) -> Result<T, F>) -> Result<T, F> {
    return match value {
        Ok(item) => Ok(item)
        Err(error) => fallback(error)
    }
}
