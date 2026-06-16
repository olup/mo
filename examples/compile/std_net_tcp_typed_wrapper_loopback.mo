import * as net from "std/net"

fn main() -> Int {
    let listener = net.tcp_listener_new(16)
    let port = net.tcp_listener_port(listener)
    if port > 0 {
        let client = net.tcp_stream_connect_loopback(port)
        let server = net.tcp_listener_accept(listener)
        let wrote = net.tcp_stream_write(client, "A")
        if wrote == 1 {
            let byte = net.tcp_stream_read_byte(server)
            if byte == 65 {
                return 42
            }
            return 4
        }
        return 3
    }
    return 1
}
