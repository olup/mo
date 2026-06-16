import * as channel from "std/channel"
import * as String from "std/string"
import * as thread from "std/thread"

fn main() -> Int {
    let ch: channel.Channel<String> = channel.new<String>()
    let worker_ch: channel.Channel<String> = channel.clone<String>(ch)
    let message = String.concat("hello", " channel")

    let handle = thread.spawn(move fn() {
        channel.send(worker_ch, message)
    })

    let value = channel.recv<String>(ch)
    let joined = thread.join(handle)
    channel.close<String>(ch)
    channel.destroy<String>(ch)

    if joined == 0 {
        if value == "hello channel" {
            return 42
        }
    }
    return 1
}
