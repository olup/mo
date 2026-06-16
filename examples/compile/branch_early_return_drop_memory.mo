import * as core from "core/unsafe"
import * as String from "std/string"

fn branch(flag: Bool) -> Int {
    let outer = String.from("outer")
    if flag {
        let first = String.from("first")
        let second = String.from("second")
        return 7
    }
    return String.len(outer)
}

fn main() -> Int {
    let before = core.mem_live_bytes()
    let result = branch(true)
    let after = core.mem_live_bytes()
    if result == 7 {
        if after == before {
            return 42
        }
        return 2
    }
    return 1
}
