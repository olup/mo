import * as core from "core/unsafe"
import * as shared from "std/shared"

fn exercise() -> Int {
    let one: shared.Shared<Int> = shared.new_int(40)
    let two: shared.Shared<Int> = shared.clone_int(one)
    shared.set_int(two, 42)
    return shared.get_int(one) + shared.get_int(two)
}

fn main() -> Int {
    let alloc0 = core.mem_alloc_count()
    let free0 = core.mem_free_count()
    let result = exercise()
    let alloc1 = core.mem_alloc_count()
    let free1 = core.mem_free_count()
    if result != 84 {
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
