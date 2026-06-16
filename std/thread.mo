import * as core from "core/unsafe"

pub struct JoinHandle {
    raw: Int
}

pub fn spawn(task: fn() -> ()) -> JoinHandle {
    let raw = core.thread_spawn(task)
    return JoinHandle { raw: raw }
}

pub fn join(handle: JoinHandle) -> Int {
    return core.thread_join(handle.raw)
}
