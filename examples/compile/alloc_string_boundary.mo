import * as alloc_string from "alloc/string"
import * as bytes from "std/bytes"

fn main() -> Int {
    let text = alloc_string.concat("mo", alloc_string.from_byte(33))
    if bytes.string_load8(text, 0) == 109 {
        if bytes.string_load8(text, 2) == 33 {
            return 42
        }
    }
    return 1
}
