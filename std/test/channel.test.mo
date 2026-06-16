import * as channel from "std/channel"
import * as String from "std/string"

test "std channel int send recv" {
    let ch = channel.int()
    assert(channel.send(ch, 42) == 0)
    assert(channel.recv(ch) == 42)
    assert(channel.close(ch) == 0)
    assert(channel.destroy(ch) == 0)
}

test "std channel string copies value" {
    let ch = channel.string()
    assert(channel.send_string_ref(ch, "hello") == 0)
    let value: String = channel.recv(ch)
    assert(value == "hello")
    assert(channel.close(ch) == 0)
    assert(channel.destroy(ch) == 0)
}
