import * as String from "std/string"
import * as vec from "std/vec"

test "std vec int stores indexed values" {
    let values: vec.Vec<Int> = vec.new<Int>()
    assert(vec.length<Int>(values) == 0)
    assert(vec.capacity<Int>(values) == 0)
    assert(vec.data<Int>(values) == 0)
    vec.push<Int>(values, 10)
    vec.push<Int>(values, 32)
    assert(vec.length<Int>(values) == 2)
    assert(vec.capacity<Int>(values) >= 2)
    assert(vec.data<Int>(values) > 0)
    assert(vec.get<Int>(values, 0) == 10)
    assert(vec.get<Int>(values, 1) == 32)
    assert(vec.get<Int>(values, 2) == 0)
    vec.destroy<Int>(values)
}

test "std vec string stores owned values" {
    let values: vec.Vec<String> = vec.new<String>()
    vec.push<String>(values, String.from("alpha"))
    vec.push<String>(values, String.from("beta"))
    assert(vec.length<String>(values) == 2)
    assert(vec.capacity<String>(values) >= 2)
    assert(vec.data<String>(values) > 0)
    assert(vec.get<String>(values, 0) == "alpha")
    assert(vec.get<String>(values, 1) == "beta")
    assert(String.len(vec.get<String>(values, 2)) == 0)
    vec.destroy<String>(values)
}

fn first_handler(client: Int, context: &Str) -> Int {
    return 7
}

fn second_handler(client: Int, context: &Str) -> Int {
    return 42
}

test "std vec handler stores callback values" {
    let handlers: vec.Vec<fn(Int, &Str) -> Int> = vec.new<fn(Int, &Str) -> Int>()
    vec.push<fn(Int, &Str) -> Int>(handlers, first_handler)
    vec.push<fn(Int, &Str) -> Int>(handlers, second_handler)
    let handler = vec.get<fn(Int, &Str) -> Int>(handlers, 1)
    assert(vec.length<fn(Int, &Str) -> Int>(handlers) == 2)
    assert(vec.capacity<fn(Int, &Str) -> Int>(handlers) >= 2)
    assert(vec.data<fn(Int, &Str) -> Int>(handlers) > 0)
    assert(handler(1, "ctx") == 42)
    vec.destroy<fn(Int, &Str) -> Int>(handlers)
}
