enum Choice {
    Number(Int)
    Empty
}

fn main() -> Int {
    let result: Choice = Number(42)
    let code = 0
    match result {
        Number(value) => code = value
        Empty => code = 1
    }
    return code
}
