import * as core from "core/unsafe"
import * as task from "std/task"

fn no_op(ptr: Int) {
    if ptr != 0 {
        core.store64(ptr, 0, 1)
    }
}

fn main() -> Int {
    let queue = task.queue4_int(no_op)
    let closed = task.close_int(queue)
    let rejected = task.submit_int(queue, 0)
    let joined = task.join_queue_int(queue)
    let destroyed = task.destroy_queue_int(queue)
    if closed == 0 {
        if rejected == 0 - 1 {
            if joined == 0 {
                if destroyed == 0 {
                    return 42
                }
            }
        }
    }
    return 1
}
