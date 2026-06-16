import * as core from "core/unsafe"

fn exercise() -> Int {
    let ptr: *mut Byte = core.alloc_ptr(2)
    core.store8_ptr(ptr, 0, 40)
    core.store8_ptr(ptr, 1, 2)
    let total = core.load8_ptr(ptr, 0) + core.load8_ptr(ptr, 1)
    core.free_ptr(ptr)
    return total
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
    if alloc1 < alloc0 + 1 {
        return 2
    }
    if free1 < free0 + 1 {
        return 3
    }
    return 42
}
