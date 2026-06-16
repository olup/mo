fn score(value: Float64) -> Int {
    if value > 10.0 && value < 11.0 {
        return 42
    }
    return 1
}

fn main() -> Int {
    let base: Float64 = 2.5
    let doubled: Float64 = base * 4.0
    let adjusted: Float64 = doubled + 0.75 - 0.25
    return score(adjusted)
}
