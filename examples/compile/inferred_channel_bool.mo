import * as channel from "std/channel"
import * as thread from "std/thread"

fn main() -> Int {
    let ch: channel.Channel<Bool> = channel.new()
    let worker_ch: channel.Channel<Bool> = channel.clone(ch)
    let handle = thread.spawn(move fn() {
        channel.send(worker_ch, true)
    })

    let value: Bool = channel.recv(ch)
    let joined = thread.join(handle)
    channel.close(ch)
    channel.destroy(ch)

    if joined == 0 {
        if value {
            return 42
        }
    }
    return 1
}
