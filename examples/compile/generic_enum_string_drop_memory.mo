import * as core from "core/unsafe"
import * as String from "std/string"

enum Maybe<T> {
    Some(T)
    None
}

fn build() -> Int {
    let value: Maybe<String> = Some(String.from("held"))
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
