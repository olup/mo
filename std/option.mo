pub enum Option<T> {
    Some(T)
    None
}

pub fn is_some<T>(value: Option<T>) -> Bool {
    return match value {
        Some(item) => true
        None => false
    }
}

pub fn is_none<T>(value: Option<T>) -> Bool {
    return match value {
        Some(item) => false
        None => true
    }
}

pub fn unwrap_or<T>(value: Option<T>, fallback: T) -> T {
    return match value {
        Some(item) => item
        None => fallback
    }
}

pub fn map<T, U>(value: Option<T>, mapper: fn(T) -> U) -> Option<U> {
    return match value {
        Some(item) => Some(mapper(item))
        None => None
    }
}

pub fn and_then<T, U>(value: Option<T>, mapper: fn(T) -> Option<U>) -> Option<U> {
    return match value {
        Some(item) => mapper(item)
        None => None
    }
}

pub fn or_else<T>(value: Option<T>, fallback: fn() -> Option<T>) -> Option<T> {
    return match value {
        Some(item) => Some(item)
        None => fallback()
    }
}
