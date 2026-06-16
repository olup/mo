import * as channel from "std/channel"
import * as thread from "std/thread"

fn main() -> Int {
    let ch: channel.Channel<Int> = channel.new<Int>()
    let worker_ch: channel.Channel<Int> = channel.clone<Int>(ch)
    let handle = thread.spawn(move fn() {
        channel.send(worker_ch, 42)
    })

    let value = channel.recv<Int>(ch)
    let joined = thread.join(handle)
    channel.close<Int>(ch)
    channel.destroy<Int>(ch)

    if joined == 0 {
        if value == 42 {
            return 42
        }
    }
    return 1
}
