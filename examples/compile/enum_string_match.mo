import * as String from "std/string"

enum TextResult {
    Text(String)
    Missing
}

fn main() -> Int {
    let result: TextResult = Text(String.from("hello"))
    let message = match result {
        Text(value) => value
        Missing => String.from("missing")
    }
    print(message)
    return 0
}
