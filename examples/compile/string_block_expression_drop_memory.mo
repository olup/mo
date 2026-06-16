import * as core from "core/unsafe"
import * as String from "std/string"

fn build() -> Int {
    let value = {
        let prefix = String.from("A")
        String.concat(prefix, "da")
    }
    return String.len(value)
}

fn direct_return() -> String {
    return {
        String.from("Grace")
    }
}

fn run_all() -> Int {
    let len = build()
    let returned = direct_return()
    return len + String.len(returned)
}

fn main() -> Int {
    let before = core.mem_live_bytes()
    let total = run_all()
    let after = core.mem_live_bytes()
    if total != 8 {
        return 1
    }
    if after != before {
        return 2
    }
    return 42
}
