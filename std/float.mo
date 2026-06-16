pub fn to_int(value: Float64) -> Int {
    if value < 0.0 {
        return 0 - to_int(0.0 - value)
    }
    let mut result = 0
    let mut next = 1
    while next * 1.0 <= value {
        result = next
        next += 1
    }
    return result
}

pub fn clamp(value: Float64, min: Float64, max: Float64) -> Float64 {
    if value < min {
        return min
    }
    if value > max {
        return max
    }
    return value
}
