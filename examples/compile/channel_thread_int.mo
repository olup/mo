import * as channel from "std/channel"
import * as thread from "std/thread"

fn main() -> Int {
    let ch = channel.int()
    let worker_ch = channel.clone(ch)

    let handle = thread.spawn(move fn() {
        channel.send(worker_ch, 42)
    })

    let value = channel.recv(ch)
    let joined = thread.join(handle)
    channel.close(ch)
    channel.destroy(ch)

    if joined == 0 {
        if value == 42 {
            return 42
        }
    }
    return 1
}
