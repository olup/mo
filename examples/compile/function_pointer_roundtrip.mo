import * as core from "core/unsafe"

fn task() {
    print("task")
}

fn main() -> Int {
    let ptr = core.alloc(8)
    let task_ptr = core.function_ptr(task)
    core.store64(ptr, 0, task_ptr)
    let loaded_ptr = core.load64(ptr, 0)
    let loaded_task = core.function_from_ptr(loaded_ptr)
    loaded_task()
    core.free(ptr)
    return 42
}
