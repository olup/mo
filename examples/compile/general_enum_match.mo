enum ParseState {
    Done(Int)
    Failed(Int)
    Empty
}

fn main() -> Int {
    let state: ParseState = Failed(42)
    return match state {
        Done(value) => value
        Failed(code) => code
        Empty => 0
    }
}
