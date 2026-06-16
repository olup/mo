import * as core from "core/unsafe"
import * as String from "std/string"

fn main() -> Int {
    let message = String.new("ok\n")
    return core.write(1, message)
}
