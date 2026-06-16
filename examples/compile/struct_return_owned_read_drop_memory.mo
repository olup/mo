import * as core from "core/unsafe"
import * as String from "std/string"

struct Report {
    len: Int
    copy: String
}

fn make(value: String) -> Report {
    return Report {
        len: String.len(value)
        copy: String.from("ok")
    }
}

fn run() -> Int {
    let report = make(String.from("hello"))
    return report.len + String.len(report.copy)
}

fn main() -> Int {
    let before = core.mem_live_bytes()
    let total = run()
    let after = core.mem_live_bytes()
    if total != 7 {
        return 1
    }
    if after != before {
        return 2
    }
    return 42
}
