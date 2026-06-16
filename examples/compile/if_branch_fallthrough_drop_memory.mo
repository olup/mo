import * as core from "core/unsafe"
import * as String from "std/string"

fn branch(flag: Bool) -> Int {
    if flag {
        let inner = String.from("branch")
        if String.len(inner) != 6 {
            return 1
        }
    }
    return 42
}

fn main() -> Int {
    let before = core.mem_live_bytes()
    let result = branch(true)
    let after = core.mem_live_bytes()
    if result != 42 {
        return result
    }
    if after != before {
        return 2
    }
    return 42
}
