import * as core from "core/unsafe"
import * as String from "std/string"

fn choose(flag: Bool) -> Int {
    let value = if flag {
        let prefix = String.from("A")
        String.concat(prefix, "da")
    } else {
        let prefix = String.from("G")
        String.concat(prefix, "race")
    }
    return String.len(value)
}

fn main() -> Int {
    let before = core.mem_live_bytes()
    let first = choose(true)
    let second = choose(false)
    let after = core.mem_live_bytes()
    if first != 3 {
        return 1
    }
    if second != 5 {
        return 2
    }
    if after != before {
        return 3
    }
    return 42
}
