import * as buffer from "std/buffer"
import * as String from "std/string"

fn main() -> Int {
    let out = buffer.new(1)
    let one = buffer.append(out, "hello")
    let two = buffer.append(out, "!")
    let text = buffer.finish(out)
    if one != 5 {
        return 1
    }
    if two != 6 {
        return 2
    }
    if String.len(text) != 6 {
        return 3
    }
    if text != "hello!" {
        return 4
    }
    return 42
}
