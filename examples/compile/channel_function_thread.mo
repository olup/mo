import * as channel from "std/channel"
import * as thread from "std/thread"

fn named_job() {
    print("named job")
}

fn main() -> Int {
    let ch: channel.Channel<fn() -> ()> = channel.new()
    let worker_ch: channel.Channel<fn() -> ()> = channel.clone(ch)

    let handle = thread.spawn(move fn() {
        let first: fn() -> () = channel.recv(worker_ch)
        first()

        let second: fn() -> () = channel.recv(worker_ch)
        second()
    })

    channel.send(ch, named_job)
    channel.send(ch, fn() {
        print("closure job")
    })

    let joined = thread.join(handle)
    channel.close(ch)
    channel.destroy(ch)

    if joined == 0 {
        return 42
    }
    return 1
}
