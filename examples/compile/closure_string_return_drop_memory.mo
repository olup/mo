import * as core from "core/unsafe"
import * as String from "std/string"

fn apply(callback: fn() -> String) -> String {
    return callback()
}

fn produce() -> String {
    let make = fn() -> String {
        return String.from("Ada")
    }
    return apply(make)
}

fn measure() -> Int {
    let before = core.mem_live_bytes()
    let value = produce()
    if String.len(value) != 3 {
        return 100
    }
    return core.mem_live_bytes() - before
}

fn main() -> Int {
    return measure()
}
