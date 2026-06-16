import * as String from "std/string"

fn greeting() -> String {
    return String.from("hello")
}

fn main() -> Int {
    let message = greeting()
    print(message)
    return 0
}
