import * as core from "core/unsafe"
import * as task from "std/task"

fn no_op(value: Int) {
    if value != 0 {
        core.store64(value, 0, 1)
    }
}

fn scoped_queue() -> Int {
    let queue = task.queue4_int(no_op)
    return 7
}

fn main() -> Int {
    let alloc0 = core.mem_alloc_count()
    let free0 = core.mem_free_count()
    let value = scoped_queue()
    let alloc1 = core.mem_alloc_count()
    let free1 = core.mem_free_count()

    if value != 7 {
        return 2
    }
    if alloc1 <= alloc0 {
        return 3
    }
    if free1 < free0 + 4 {
        return 4
    }
    return 42
}
