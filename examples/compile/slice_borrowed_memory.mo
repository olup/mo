import * as core from "core/unsafe"
import * as slice from "std/slice"

fn exercise() -> Int {
    let whole = slice.from_str("borrowed")
    let view = slice.subslice(whole, 1, 4)
    if slice.len(view) == 4 {
        if view[0] == 111 {
            if view[3] == 111 {
                return 42
            }
        }
    }
    return 1
}

fn main() -> Int {
    let alloc0 = core.mem_alloc_count()
    let free0 = core.mem_free_count()
    let result = exercise()
    let alloc1 = core.mem_alloc_count()
    let free1 = core.mem_free_count()
    if result != 42 {
        return result
    }
    if alloc1 > alloc0 + 2 {
        return 2
    }
    if free1 > free0 + 2 {
        return 3
    }
    return 42
}
