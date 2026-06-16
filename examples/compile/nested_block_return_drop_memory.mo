import * as core from "core/unsafe"
import * as String from "std/string"

fn nested() -> Int {
    let outer = String.from("outer")
    let value = {
        let inner = String.from("inner")
        return String.len(outer)
    }
    return value
}

fn main() -> Int {
    let before = core.mem_live_bytes()
    let result = nested()
    let after = core.mem_live_bytes()
    if result == 5 {
        if after == before {
            return 42
        }
        return 2
    }
    return 1
}
