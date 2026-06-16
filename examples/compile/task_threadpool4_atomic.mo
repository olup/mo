import * as task from "std/task"

fn job() {}

fn main() -> Int {
    let pool = task.threadpool4()

    let joined = task.run4(
        pool,
        job,
        fn() {},
        job,
        fn() {}
    )

    if joined == 0 {
        return 42
    }
    return 1
}
