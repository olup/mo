import * as core from "core/unsafe"
import * as String from "std/string"

fn replace() -> Int {
    let mut value = String.from("first")
    value = String.from("second")
    return String.len(value)
}

fn main() -> Int {
    let before = core.mem_live_bytes()
    let len = replace()
    let after = core.mem_live_bytes()
    if len == 6 {
        if after == before {
            return 42
        }
        return 2
    }
    return 1
}
