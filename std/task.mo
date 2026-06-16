import * as core from "core/unsafe"
import * as channel from "std/channel"
import { Channel } from "std/channel"

pub struct ThreadPool4 {
    raw: Int
}

pub struct TaskQueue4 {
    mutex: Int
    not_empty: Int
    not_full: Int
    cell: Int
    worker1: Int
    worker2: Int
    worker3: Int
    worker4: Int
}

pub struct TaskQueue4Int {
    mutex: Int
    not_empty: Int
    not_full: Int
    cell: Int
    worker1: Int
    worker2: Int
    worker3: Int
    worker4: Int
}

pub struct JoinHandle {
    raw: Int
}

pub fn spawn(task: fn() -> ()) -> JoinHandle {
    let raw = core.thread_spawn(task)
    return JoinHandle { raw: raw }
}

pub fn join(task: JoinHandle) -> Int {
    return core.thread_join(task.raw)
}

pub fn threadpool4() -> ThreadPool4 {
    return ThreadPool4 { raw: 4 }
}

pub fn run4(pool: ThreadPool4, one: fn() -> (), two: fn() -> (), three: fn() -> (), four: fn() -> ()) -> Int {
    if pool.raw != 4 {
        return 0 - 2
    }
    let task1 = spawn(one)
    let task2 = spawn(two)
    let task3 = spawn(three)
    let task4 = spawn(four)
    let joined1 = join(task1)
    let joined2 = join(task2)
    let joined3 = join(task3)
    let joined4 = join(task4)
    if joined1 == 0 {
        if joined2 == 0 {
            if joined3 == 0 {
                return joined4
            }
        }
    }
    return 0 - 1
}

fn stop_worker() {}

pub fn queue4() -> TaskQueue4 {
    let jobs: channel.Channel<fn() -> ()> = channel.new()
    let worker_jobs1: channel.Channel<fn() -> ()> = channel.clone(jobs)
    let worker_jobs2: channel.Channel<fn() -> ()> = channel.clone(jobs)
    let worker_jobs3: channel.Channel<fn() -> ()> = channel.clone(jobs)
    let worker_jobs4: channel.Channel<fn() -> ()> = channel.clone(jobs)

    let worker1 = spawn(move fn() {
        while true {
            let job: fn() -> () = channel.recv(worker_jobs1)
            if core.function_ptr(job) == core.function_ptr(stop_worker) {
                return
            }
            job()
        }
    })
    let worker2 = spawn(move fn() {
        while true {
            let job: fn() -> () = channel.recv(worker_jobs2)
            if core.function_ptr(job) == core.function_ptr(stop_worker) {
                return
            }
            job()
        }
    })
    let worker3 = spawn(move fn() {
        while true {
            let job: fn() -> () = channel.recv(worker_jobs3)
            if core.function_ptr(job) == core.function_ptr(stop_worker) {
                return
            }
            job()
        }
    })
    let worker4 = spawn(move fn() {
        while true {
            let job: fn() -> () = channel.recv(worker_jobs4)
            if core.function_ptr(job) == core.function_ptr(stop_worker) {
                return
            }
            job()
        }
    })

    return TaskQueue4 {
        mutex: jobs.mutex,
        not_empty: jobs.not_empty,
        not_full: jobs.not_full,
        cell: jobs.cell,
        worker1: worker1.raw,
        worker2: worker2.raw,
        worker3: worker3.raw,
        worker4: worker4.raw
    }
}

pub fn submit(queue: &TaskQueue4, job: fn() -> ()) -> Int {
    let jobs: Channel<fn() -> ()> = Channel {
        mutex: queue.mutex,
        not_empty: queue.not_empty,
        not_full: queue.not_full,
        cell: queue.cell
    }
    return channel.send(jobs, job)
}

pub fn close(queue: &TaskQueue4) -> Int {
    let jobs: Channel<fn() -> ()> = Channel {
        mutex: queue.mutex,
        not_empty: queue.not_empty,
        not_full: queue.not_full,
        cell: queue.cell
    }
    let stopped1 = channel.send(jobs, stop_worker)
    let stopped2 = channel.send(jobs, stop_worker)
    let stopped3 = channel.send(jobs, stop_worker)
    let stopped4 = channel.send(jobs, stop_worker)
    let closed = channel.close(jobs)
    if stopped1 != 0 {
        return stopped1
    }
    if stopped2 != 0 {
        return stopped2
    }
    if stopped3 != 0 {
        return stopped3
    }
    if stopped4 != 0 {
        return stopped4
    }
    return closed
}

pub fn join_queue(queue: &TaskQueue4) -> Int {
    let joined1 = core.thread_join(queue.worker1)
    let joined2 = core.thread_join(queue.worker2)
    let joined3 = core.thread_join(queue.worker3)
    let joined4 = core.thread_join(queue.worker4)
    if joined1 == 0 {
        if joined2 == 0 {
            if joined3 == 0 {
                return joined4
            }
        }
    }
    return 0 - 1
}

pub fn destroy_queue(queue: &TaskQueue4) -> Int {
    let jobs: Channel<fn() -> ()> = Channel {
        mutex: queue.mutex,
        not_empty: queue.not_empty,
        not_full: queue.not_full,
        cell: queue.cell
    }
    return channel.destroy(jobs)
}

pub fn queue4_int(handler: fn(Int) -> ()) -> TaskQueue4Int {
    let jobs: channel.Channel<Int> = channel.new()
    let worker_jobs1: channel.Channel<Int> = channel.clone(jobs)
    let worker_jobs2: channel.Channel<Int> = channel.clone(jobs)
    let worker_jobs3: channel.Channel<Int> = channel.clone(jobs)
    let worker_jobs4: channel.Channel<Int> = channel.clone(jobs)
    let handler_ptr = core.function_ptr_int(handler)

    let worker1 = spawn(move fn() {
        let run = core.function_from_ptr_int(handler_ptr)
        while true {
            let job = channel.recv(worker_jobs1)
            if job == 0 {
                return
            }
            run(job)
        }
    })
    let worker2 = spawn(move fn() {
        let run = core.function_from_ptr_int(handler_ptr)
        while true {
            let job = channel.recv(worker_jobs2)
            if job == 0 {
                return
            }
            run(job)
        }
    })
    let worker3 = spawn(move fn() {
        let run = core.function_from_ptr_int(handler_ptr)
        while true {
            let job = channel.recv(worker_jobs3)
            if job == 0 {
                return
            }
            run(job)
        }
    })
    let worker4 = spawn(move fn() {
        let run = core.function_from_ptr_int(handler_ptr)
        while true {
            let job = channel.recv(worker_jobs4)
            if job == 0 {
                return
            }
            run(job)
        }
    })

    return TaskQueue4Int {
        mutex: jobs.mutex,
        not_empty: jobs.not_empty,
        not_full: jobs.not_full,
        cell: jobs.cell,
        worker1: worker1.raw,
        worker2: worker2.raw,
        worker3: worker3.raw,
        worker4: worker4.raw
    }
}

pub fn submit_int(queue: &TaskQueue4Int, job: Int) -> Int {
    let jobs: Channel<Int> = Channel {
        mutex: queue.mutex,
        not_empty: queue.not_empty,
        not_full: queue.not_full,
        cell: queue.cell
    }
    return channel.send(jobs, job)
}

pub fn close_int(queue: &TaskQueue4Int) -> Int {
    let jobs: Channel<Int> = Channel {
        mutex: queue.mutex,
        not_empty: queue.not_empty,
        not_full: queue.not_full,
        cell: queue.cell
    }
    let stopped1 = channel.send(jobs, 0)
    let stopped2 = channel.send(jobs, 0)
    let stopped3 = channel.send(jobs, 0)
    let stopped4 = channel.send(jobs, 0)
    let closed = channel.close(jobs)
    if stopped1 != 0 {
        return stopped1
    }
    if stopped2 != 0 {
        return stopped2
    }
    if stopped3 != 0 {
        return stopped3
    }
    if stopped4 != 0 {
        return stopped4
    }
    return closed
}

pub fn join_queue_int(queue: &TaskQueue4Int) -> Int {
    let joined1 = core.thread_join(queue.worker1)
    let joined2 = core.thread_join(queue.worker2)
    let joined3 = core.thread_join(queue.worker3)
    let joined4 = core.thread_join(queue.worker4)
    if joined1 == 0 {
        if joined2 == 0 {
            if joined3 == 0 {
                return joined4
            }
        }
    }
    return 0 - 1
}

pub fn destroy_queue_int(queue: &TaskQueue4Int) -> Int {
    let jobs: Channel<Int> = Channel {
        mutex: queue.mutex,
        not_empty: queue.not_empty,
        not_full: queue.not_full,
        cell: queue.cell
    }
    return channel.destroy(jobs)
}
