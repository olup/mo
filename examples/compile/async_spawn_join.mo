import * as async from "std/async"

fn main() -> Int {
    let task = async.spawn(move fn() {
        print("async task")
    })
    let joined = async.join(task)
    if joined == 0 {
        return 42
    }
    return 1
}

