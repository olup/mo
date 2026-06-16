import * as channel from "std/channel"
import * as String from "std/string"
import * as thread from "std/thread"

fn main() -> Int {
    let ch: channel.Channel<String> = channel.new()
    let worker_ch: channel.Channel<String> = channel.clone(ch)
    let message = String.concat("hello", " inferred")

    let handle = thread.spawn(move fn() {
        channel.send(worker_ch, message)
    })

    let value: String = channel.recv(ch)
    let joined = thread.join(handle)
    channel.close(ch)
    channel.destroy(ch)

    if joined == 0 {
        if value == "hello inferred" {
            return 42
        }
    }
    return 1
}
