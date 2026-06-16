enum PairResult {
    Pair(Int, Int)
    Empty
}

fn main() -> Int {
    let result: PairResult = Pair(20, 22)
    return match result {
        Pair(left, right) => left + right
        Empty => 0
    }
}
