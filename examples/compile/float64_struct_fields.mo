struct Vec2 {
    x: Float64
    y: Float64
}

fn length2(value: Vec2) -> Float64 {
    return value.x * value.x + value.y * value.y
}

fn main() -> Int {
    let value = Vec2 { x: 3.0, y: 4.0 }
    let squared = length2(value)
    if squared == 25.0 {
        return 42
    }
    return 1
}
