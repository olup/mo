struct Point {
    x: Int
    y: Int
}

fn main() -> Int {
    let point = Point { x: 20, y: 22 }
    return point.x + point.y
}
