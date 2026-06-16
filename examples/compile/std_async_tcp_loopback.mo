import * as async_tcp from "std/async_tcp"
import * as event from "std/event"
import * as net from "std/net"

fn main() -> Int {
    let loop = event.new()
    let listener = net.tcp_listener_new(16)
    let port = net.tcp_listener_port(listener)
    if port > 0 {
        let client = net.tcp_stream_connect_loopback(port)
        let server = async_tcp.accept(loop, listener)
        let wrote = async_tcp.write(loop, client, "C")
        if wrote == 1 {
            let byte = async_tcp.read_byte(loop, server)
            if byte == 67 {
                return 42
            }
            return 4
        }
        return 3
    }
    return 1
}
