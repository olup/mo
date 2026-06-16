import * as io from "std/io"
import * as task from "std/task"

fn job() {
    io.write_fd(1, "job\n")
}

fn main() -> Int {
    let queue = task.queue4()
    let submitted1 = task.submit(queue, job)
    let submitted2 = task.submit(queue, job)
    let submitted3 = task.submit(queue, job)
    let submitted4 = task.submit(queue, job)
    let submitted5 = task.submit(queue, job)
    let submitted6 = task.submit(queue, job)
    let closed = task.close(queue)
    let joined = task.join_queue(queue)
    task.destroy_queue(queue)

    if submitted1 == 0 {
        if submitted2 == 0 {
            if submitted3 == 0 {
                if submitted4 == 0 {
                    if submitted5 == 0 {
                        if submitted6 == 0 {
                            if closed == 0 {
                                if joined == 0 {
                                    return 42
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
