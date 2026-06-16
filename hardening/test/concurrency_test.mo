import * as async from "std/async"
import * as channel from "std/channel"
import * as core from "core/unsafe"
import * as String from "std/string"
import * as sync from "std/sync"
import * as task from "std/task"
import * as thread from "std/thread"

fn no_op_job() {}

fn identity(value: Int) -> Int {
    return value
}

fn increment(value: Int) -> Int {
    return value + 1
}

fn plus_two(value: Int) -> Int {
    return value + 2
}

fn run_callback(callback: fn(Int) -> Int, value: Int) -> Int {
    return callback(value)
}

struct HandlerBox {
    pub handler: fn(Int) -> Int
}

struct Payload {
    pub label: String
    pub count: Int
}

fn get(box: &mut HandlerBox, path: &Str, handler: fn(Int) -> Int) -> Int {
    if path == "/inc" {
        box.handler = handler
        return 1
    }
    return 0
}

test "mutex protects raw shared counter across four threads" {
    let lock = sync.mutex()
    let raw = sync.raw_mutex(lock)
    let cell = core.alloc(8)
    core.store64(cell, 0, 0)

    let one = thread.spawn(move fn() {
        let mut index = 0
        while index < 50 {
            sync.lock_raw(raw)
            let current = core.load64(cell, 0)
            core.store64(cell, 0, current + 1)
            sync.unlock_raw(raw)
            index = index + 1
        }
    })
    let two = thread.spawn(move fn() {
        let mut index = 0
        while index < 50 {
            sync.lock_raw(raw)
            let current = core.load64(cell, 0)
            core.store64(cell, 0, current + 1)
            sync.unlock_raw(raw)
            index = index + 1
        }
    })
    let three = thread.spawn(move fn() {
        let mut index = 0
        while index < 50 {
            sync.lock_raw(raw)
            let current = core.load64(cell, 0)
            core.store64(cell, 0, current + 1)
            sync.unlock_raw(raw)
            index = index + 1
        }
    })
    let four = thread.spawn(move fn() {
        let mut index = 0
        while index < 50 {
            sync.lock_raw(raw)
            let current = core.load64(cell, 0)
            core.store64(cell, 0, current + 1)
            sync.unlock_raw(raw)
            index = index + 1
        }
    })

    assert(thread.join(one) == 0)
    assert(thread.join(two) == 0)
    assert(thread.join(three) == 0)
    assert(thread.join(four) == 0)
    assert(core.load64(cell, 0) == 200)

    core.free(cell)
    sync.destroy(lock)
}



test "rwlock allows read after write without losing protected value" {
    let rw = sync.rwlock()
    let cell = core.alloc(8)

    assert(sync.write_lock(rw) == 0)
    core.store64(cell, 0, 41)
    core.store64(cell, 0, core.load64(cell, 0) + 1)
    assert(sync.rw_unlock(rw) == 0)

    assert(sync.read_lock(rw) == 0)
    let value = core.load64(cell, 0)
    assert(sync.rw_unlock(rw) == 0)

    assert(value == 42)
    core.free(cell)
    sync.rw_destroy(rw)
}

test "int channel transfers from producer thread and close is observable" {
    let ch = channel.int()
    let worker_ch = channel.clone(ch)

    let producer = thread.spawn(move fn() {
        channel.send(worker_ch, 42)
    })

    let value = channel.recv(ch)
    assert(thread.join(producer) == 0)
    assert(value == 42)
    assert(channel.close(ch) == 0)
    assert(channel.recv(ch) == 0 - 1)
    assert(channel.send(ch, 1) == 0 - 1)
    channel.destroy(ch)
}

test "bool channel transfers true and closed receive falls back false" {
    let ch: channel.Channel<Bool> = channel.new()
    let worker_ch: channel.Channel<Bool> = channel.clone(ch)

    let producer = thread.spawn(move fn() {
        channel.send(worker_ch, true)
    })

    let value: Bool = channel.recv(ch)
    assert(thread.join(producer) == 0)
    assert(value == true)
    assert(channel.close(ch) == 0)
    assert(channel.recv(ch) == false)
    channel.destroy(ch)
}



test "function channel transfers named and closure callbacks" {
    let ch: channel.Channel<fn() -> ()> = channel.new()
    let worker_ch: channel.Channel<fn() -> ()> = channel.clone(ch)

    let consumer = thread.spawn(move fn() {
        let first: fn() -> () = channel.recv(worker_ch)
        first()
        let second: fn() -> () = channel.recv(worker_ch)
        second()
    })

    assert(channel.send(ch, no_op_job) == 0)
    assert(channel.send(ch, fn() {}) == 0)
    assert(thread.join(consumer) == 0)
    assert(channel.close(ch) == 0)
    channel.destroy(ch)
}

test "task queue accepts more jobs than workers and shuts down" {
    let queue = task.queue4()
    assert(task.submit(queue, no_op_job) == 0)
    assert(task.submit(queue, no_op_job) == 0)
    assert(task.submit(queue, no_op_job) == 0)
    assert(task.submit(queue, no_op_job) == 0)
    assert(task.submit(queue, no_op_job) == 0)
    assert(task.submit(queue, no_op_job) == 0)
    assert(task.close(queue) == 0)
    assert(task.join_queue(queue) == 0)
    task.destroy_queue(queue)
}

test "async spawn joins moved string closure" {
    let message = String.concat("async", " capture")
    let handle = async.spawn(move fn() {
        if message == "async capture" {
            no_op_job()
        }
    })
    assert(async.join(handle) == 0)
}

test "thread move closure can own heap-backed struct fields" {
    let payload = Payload { label: String.from("worker"), count: 42 }
    let handle = thread.spawn(move fn() {
        if payload.label == "worker" {
            if payload.count == 42 {
                no_op_job()
            }
        }
    })
    assert(thread.join(handle) == 0)
}

test "callbacks can be named functions closures and function fields" {
    let direct = run_callback(increment, 40)
    let via_closure = run_callback(fn(value: Int) -> Int {
        return value + 2
    }, 40)
    let box = HandlerBox { handler: plus_two }

    assert(direct == 41)
    assert(via_closure == 42)
    assert(box.handler(40) == 42)
}

test "method registration mutates function valued handler field" {
    let mut box = HandlerBox { handler: identity }
    assert(box.handler(41) == 41)
    assert(box.get("/inc", increment) == 1)
    assert(box.handler(41) == 42)
    assert(box.get("/missing", plus_two) == 0)
    assert(box.handler(41) == 42)
}
