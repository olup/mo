import * as core from "core/unsafe"
import * as result from "std/result"
import * as String from "std/string"

fn recover(value: String) -> result.Result<Int, Int> {
    return Ok(String.len(value) + 37)
}

fn run() -> Int {
    let value: result.Result<Int, Int> = result.or_else(Err(String.from("owned")), recover)
    return match value {
        Ok(item) => item
        Err(error) => error
    }
}

fn main() -> Int {
    let before = core.mem_live_bytes()
    let total = run()
    let after = core.mem_live_bytes()
    if total != 42 {
        return 1
    }
    if after != before {
        return 2
    }
    return 42
}
