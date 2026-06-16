import * as box from "std/box"
import * as core from "core/unsafe"

fn exercise() -> Int {
    let value: box.Box<Int> = box.new(42)
    return box.get_int(value)
}

fn main() -> Int {
    let alloc0 = core.mem_alloc_count()
    let free0 = core.mem_free_count()
    let result = exercise()
    let alloc1 = core.mem_alloc_count()
    let free1 = core.mem_free_count()
    if result != 42 {
        return 1
    }
    if alloc1 < alloc0 + 2 {
        return 2
    }
    if free1 < free0 + 2 {
        return 3
    }
    return 42
}
