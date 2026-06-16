import * as core from "core/unsafe"
import * as task from "std/task"

fn release(ptr: Int) {
    core.store64(ptr, 0, 99)
    core.free(ptr)
}

fn main() -> Int {
    let queue = task.queue4_int(release)
    let alloc0 = core.mem_alloc_count()
    let free0 = core.mem_free_count()
    let live0 = core.mem_live_bytes()

    let cell1 = core.alloc(8)
    let cell2 = core.alloc(8)
    let cell3 = core.alloc(8)
    let cell4 = core.alloc(8)
    core.store64(cell1, 0, 1)
    core.store64(cell2, 0, 2)
    core.store64(cell3, 0, 3)
    core.store64(cell4, 0, 4)

    let submitted1 = task.submit_int(queue, cell1)
    let submitted2 = task.submit_int(queue, cell2)
    let submitted3 = task.submit_int(queue, cell3)
    let submitted4 = task.submit_int(queue, cell4)
    let closed = task.close_int(queue)
    let joined = task.join_queue_int(queue)
    let destroyed = task.destroy_queue_int(queue)

    let alloc1 = core.mem_alloc_count()
    let free1 = core.mem_free_count()
    let live1 = core.mem_live_bytes()

    if submitted1 == 0 {
        if submitted2 == 0 {
            if submitted3 == 0 {
                if submitted4 == 0 {
                    if closed == 0 {
                        if joined == 0 {
                            if destroyed == 0 {
                                if alloc1 >= alloc0 + 4 {
                                    if free1 >= free0 + 4 {
                                        if live1 <= live0 {
                                            return 42
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    return 1
}
