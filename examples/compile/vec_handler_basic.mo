import * as vec from "std/vec"

fn first(client: Int, context: &Str) -> Int {
    return 7
}

fn second(client: Int, context: &Str) -> Int {
    return 42
}

fn main() -> Int {
    let handlers: vec.Vec<fn(Int, &Str) -> Int> = vec.new<fn(Int, &Str) -> Int>()
    vec.push<fn(Int, &Str) -> Int>(handlers, first)
    vec.push<fn(Int, &Str) -> Int>(handlers, second)
    let handler = vec.get<fn(Int, &Str) -> Int>(handlers, 1)
    if vec.length<fn(Int, &Str) -> Int>(handlers) == 2 {
        return handler(1, "ctx")
    }
    return 1
}
