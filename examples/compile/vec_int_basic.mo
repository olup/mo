import * as vec from "std/vec"
import * as core from "core/unsafe"

fn exercise() -> Int {
    let values: vec.Vec<Int> = vec.new<Int>()
    vec.push<Int>(values, 10)
    vec.push<Int>(values, 32)
    return vec.get<Int>(values, 0) + vec.get<Int>(values, 1) + vec.length<Int>(values)
}

fn main() -> Int {
    let alloc0 = core.mem_alloc_count()
    let free0 = core.mem_free_count()
    let result = exercise()
    let alloc1 = core.mem_alloc_count()
    let free1 = core.mem_free_count()
    if result != 44 {
        return 1
    }
    if alloc1 < alloc0 + 3 {
        return 2
    }
    if free1 < free0 + 3 {
        return 3
    }
    return 42
}
