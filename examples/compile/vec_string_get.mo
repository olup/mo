import * as vec from "std/vec"
import * as String from "std/string"
import * as core from "core/unsafe"

fn exercise() -> Int {
    let values: vec.Vec<String> = vec.new<String>()
    let one = String.concat("a", "b")
    let two = String.concat("c", "de")
    vec.push<String>(values, one)
    vec.push<String>(values, two)
    return String.len(vec.get<String>(values, 0)) + String.len(vec.get<String>(values, 1))
}

fn main() -> Int {
    let alloc0 = core.mem_alloc_count()
    let free0 = core.mem_free_count()
    let len = exercise()
    let alloc1 = core.mem_alloc_count()
    let free1 = core.mem_free_count()
    if len != 5 {
        return 1
    }
    if alloc1 < alloc0 + 5 {
        return 2
    }
    if free1 < free0 + 5 {
        return 3
    }
    return 42
}
