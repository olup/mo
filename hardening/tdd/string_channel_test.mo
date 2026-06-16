// Regression hardening spec: Channel<String> copies from a borrowed source string.

import * as channel from "std/channel"
import * as String from "std/string"
import * as thread from "std/thread"

test "string channel copies borrowed string across thread without crashing" {
    let ch: channel.Channel<String> = channel.new()
    let worker_ch: channel.Channel<String> = channel.clone(ch)
    let message = String.concat("safe", " transfer")

    let producer = thread.spawn(move fn() {
        channel.send_string_ref(worker_ch, message)
    })

    let value: String = channel.recv(ch)
    assert(thread.join(producer) == 0)
    assert(value == "safe transfer")
    assert(channel.close(ch) == 0)
    channel.destroy(ch)
}
