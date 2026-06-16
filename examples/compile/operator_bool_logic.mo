fn main() -> Int {
    let a = true
    let b = false
    if a && !b {
        if b || a {
            return 42
        }
    }
    return 1
}
