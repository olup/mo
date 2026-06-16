import * as core from "core/unsafe"
import * as String from "std/string"

struct Holder<T> {
    value: T
}

fn build() -> Int {
    let holder = Holder { value: String.from("held") }
    return 7
}

fn main() -> Int {
    let before = core.mem_live_bytes()
    let result = build()
    let after = core.mem_live_bytes()
    if result == 7 {
        if after == before {
            return 42
        }
        return 2
    }
    return 1
}
