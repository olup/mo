import * as String from "std/string"

fn takes_str(value: &Str) -> Int {
    return String.len(value)
}

fn main() -> Int {
    let view: Str = "hello"
    if takes_str(view) == 5 {
        return 42
    }
    return 1
}
