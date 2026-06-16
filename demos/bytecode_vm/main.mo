import * as core from "core/unsafe"
import * as io from "std/io"
import * as String from "std/string"
import * as task from "std/task"

struct Cpu {
    pc: Int
    acc: Int
    counter: Int
    limit: Int
    checksum: Int
    steps: Int
    halted: Int
}

fn run_vm(limit: Int) -> Cpu {
    let mut ip = 0
    let mut acc = 0
    let mut counter = 0
    let mut limit_value = limit
    let mut checksum = 0
    let mut steps = 0
    let mut halted = 0
    let mut guard = 0
    while guard < 256 {
        let op = ip
        if op == 0 {
            counter = 1
            ip += 1
        }
        if op == 1 {
            limit_value = limit
            ip += 1
        }
        if op == 2 {
            acc += counter * counter
            ip += 1
        }
        if op == 3 {
            checksum = checksum * 31 + acc + counter
            ip += 1
        }
        if op == 4 {
            counter += 1
            ip += 1
        }
        if op == 5 {
            if counter <= limit_value {
                ip = 2
            }
            if counter > limit_value {
                ip += 1
            }
        }
        if op > 5 {
            halted = 1
        }
        steps += 1
        if halted == 1 {
            return Cpu {
                pc: ip,
                acc: acc,
                counter: counter,
                limit: limit_value,
                checksum: checksum,
                steps: steps,
                halted: halted
            }
        }
        guard += 1
    }
    return Cpu {
        pc: ip,
        acc: acc,
        counter: counter,
        limit: limit_value,
        checksum: checksum,
        steps: steps,
        halted: 1
    }
}

fn vm_score(cpu: &Cpu) -> Int {
    return cpu.acc + cpu.checksum + cpu.steps
}

fn vm_job(cell: Int) {
    let limit = core.load64(cell, 0)
    let cpu = run_vm(limit)
    core.store64(cell, 8, cpu.acc)
    core.store64(cell, 16, cpu.checksum)
    core.store64(cell, 24, cpu.steps)
    core.store64(cell, 32, vm_score(cpu))
}

fn make_job(limit: Int) -> Int {
    let cell = core.alloc(40)
    core.store64(cell, 0, limit)
    core.store64(cell, 8, 0)
    core.store64(cell, 16, 0)
    core.store64(cell, 24, 0)
    core.store64(cell, 32, 0)
    return cell
}

fn run_parallel() -> Int {
    let job1 = make_job(8)
    let job2 = make_job(9)
    let job3 = make_job(10)
    let job4 = make_job(11)
    let queue = task.queue4_int(vm_job)
    let submitted1 = task.submit_int(queue, job1)
    let submitted2 = task.submit_int(queue, job2)
    let submitted3 = task.submit_int(queue, job3)
    let submitted4 = task.submit_int(queue, job4)
    let closed = task.close_int(queue)
    let joined = task.join_queue_int(queue)
    task.destroy_queue_int(queue)

    let score = core.load64(job1, 32) + core.load64(job2, 32) + core.load64(job3, 32) + core.load64(job4, 32)
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
    let single = run_vm(8)
    io.write_fd(1, "Mo bytecode VM demo\n")
    io.write_fd(1, "single acc: ")
    io.write_fd(1, String.from_int(single.acc))
    io.write_fd(1, "\n")
    io.write_fd(1, "single checksum: ")
    io.write_fd(1, String.from_int(single.checksum))
    io.write_fd(1, "\n")
    let parallel_score = run_parallel()
    io.write_fd(1, "parallel score: ")
    io.write_fd(1, String.from_int(parallel_score))
    io.write_fd(1, "\n")

    if single.acc == 204 && single.checksum == 61757734716 && single.steps == 35 && parallel_score == 1901150105803987 {
        return 42
    }
    return 1
}
