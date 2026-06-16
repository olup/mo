import * as core from "core/unsafe"
import * as task from "std/task"

fn mark(ptr: Int) {
    core.store64(ptr, 0, 1)
}

fn main() -> Int {
    let cell1 = core.alloc(8)
    let cell2 = core.alloc(8)
    let cell3 = core.alloc(8)
    let cell4 = core.alloc(8)
    core.store64(cell1, 0, 0)
    core.store64(cell2, 0, 0)
    core.store64(cell3, 0, 0)
    core.store64(cell4, 0, 0)

    let queue = task.queue4_int(mark)
    let submitted1 = task.submit_int(queue, cell1)
    let submitted2 = task.submit_int(queue, cell2)
    let submitted3 = task.submit_int(queue, cell3)
    let submitted4 = task.submit_int(queue, cell4)
    let closed = task.close_int(queue)
    let joined = task.join_queue_int(queue)
    task.destroy_queue_int(queue)

    let value = core.load64(cell1, 0) + core.load64(cell2, 0) + core.load64(cell3, 0) + core.load64(cell4, 0)
    core.free(cell1)
    core.free(cell2)
    core.free(cell3)
    core.free(cell4)
    if submitted1 == 0 {
        if submitted2 == 0 {
            if submitted3 == 0 {
                if submitted4 == 0 {
                    if closed == 0 {
                        if joined == 0 {
                            if value == 4 {
                                return 42
                            }
                        }
                    }
                }
            }
        }
    }
    return 1
}
