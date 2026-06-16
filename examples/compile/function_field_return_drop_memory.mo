import * as core from "core/unsafe"
import * as String from "std/string"

struct Handler {
    label: String
    callback: fn(Int) -> Int
}

fn inc(value: Int) -> Int {
    return value + 1
}

fn choose() -> fn(Int) -> Int {
    let handler = Handler {
        label: String.from("owned")
        callback: inc
    }
    return handler.callback
}

fn main() -> Int {
    let before = core.mem_live_bytes()
    let callback = choose()
    let after = core.mem_live_bytes()
    if callback(41) != 42 {
        return 1
    }
    if after != before {
        return 2
    }
    return 42
}
