import * as core from "core/unsafe"
import * as String from "std/string"

fn break_case() -> Int {
    let mut count = 0
    while count < 1 {
        let inner = String.from("break")
        count += 1
        break
    }
    return count
}

fn continue_case() -> Int {
    let mut count = 0
    while count < 1 {
        let inner = String.from("continue")
        count += 1
        continue
    }
    return count
}

fn main() -> Int {
    let before = core.mem_live_bytes()
    let result = break_case() + continue_case()
    let after = core.mem_live_bytes()
    if result == 2 {
        if after == before {
            return 42
        }
        return 2
    }
    return 1
}
