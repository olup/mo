import * as core from "core/unsafe"
import * as io from "std/io"
import * as String from "std/string"
import * as task from "std/task"

fn make_job(batch: Int) -> Int {
    let cell = core.alloc(48)
    core.store64(cell, 0, batch)
    core.store64(cell, 8, 0)
    core.store64(cell, 16, 0)
    core.store64(cell, 24, 0)
    core.store64(cell, 32, 0)
    core.store64(cell, 40, 0)
    return cell
}

fn analyze_batch(cell: Int) {
    let batch = core.load64(cell, 0)
    let mut index = 0
    let mut info = 0
    let mut warn = 0
    let mut error = 0
    let mut latency = 0
    let mut checksum = 0

    while index < 32 {
        let code = (batch * 17 + index * 13) % 10
        let ms = 10 + (((batch + 1) * (index + 3)) % 97)
        latency += ms
        checksum = (checksum * 31 + code * 7 + ms) % 1000000007

        if code < 6 {
            info += 1
        }
        if code >= 6 && code < 9 {
            warn += 1
        }
        if code >= 9 {
            error += 1
        }
        index += 1
    }

    core.store64(cell, 8, info)
    core.store64(cell, 16, warn)
    core.store64(cell, 24, error)
    core.store64(cell, 32, latency)
    core.store64(cell, 40, checksum)
}

fn field_sum(one: Int, two: Int, three: Int, four: Int, offset: Int) -> Int {
    return core.load64(one, offset) + core.load64(two, offset) + core.load64(three, offset) + core.load64(four, offset)
}

fn run_pipeline() -> Int {
    let job1 = make_job(0)
    let job2 = make_job(1)
    let job3 = make_job(2)
    let job4 = make_job(3)

    let queue = task.queue4_int(analyze_batch)
    let submitted1 = task.submit_int(queue, job1)
    let submitted2 = task.submit_int(queue, job2)
    let submitted3 = task.submit_int(queue, job3)
    let submitted4 = task.submit_int(queue, job4)
    let closed = task.close_int(queue)
    let joined = task.join_queue_int(queue)
    task.destroy_queue_int(queue)

    let info = field_sum(job1, job2, job3, job4, 8)
    let warn = field_sum(job1, job2, job3, job4, 16)
    let error = field_sum(job1, job2, job3, job4, 24)
    let latency = field_sum(job1, job2, job3, job4, 32)
    let checksum = field_sum(job1, job2, job3, job4, 40)
    let score = info * 3 + warn * 5 + error * 11 + latency + checksum

    core.free(job1)
    core.free(job2)
    core.free(job3)
    core.free(job4)

    if submitted1 == 0 && submitted2 == 0 && submitted3 == 0 && submitted4 == 0 && closed == 0 && joined == 0 {
        return score
    }
    return 0
}

fn main() -> Int {
    let score = run_pipeline()
    io.write_fd(1, "Mo concurrent log pipeline demo\n")
    io.write_fd(1, "aggregated score: ")
    io.write_fd(1, String.from_int(score))
    io.write_fd(1, "\n")
    if score == 1440361695 {
        return 42
    }
    return 1
}
