import * as event from "std/event"
import * as net from "std/net"

fn main() -> Int {
    let loop = event.new()
    if event.backend(loop) != 1 {
        return 1
    }
    let listener = net.tcp_listener_new(16)
    let port = net.tcp_listener_port(listener)
    if port > 0 {
        let client = net.tcp_stream_connect_loopback(port)
        let listener_ready = event.wait_listener(loop, listener)
        if listener_ready > 0 {
            let server = net.tcp_listener_accept(listener)
            let wrote = net.tcp_stream_write(client, "B")
            if wrote == 1 {
                let stream_ready = event.wait_stream(loop, server)
                if stream_ready > 0 {
                    let byte = net.tcp_stream_read_byte(server)
                    if byte == 66 {
                        return 42
                    }
                    return 6
                }
                return 5
            }
            return 4
        }
        return 3
    }
    return 2
}
