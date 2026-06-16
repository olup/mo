import * as net from "std/net"

test "std net listen port close" {
    let listener = net.tcp_listen_ephemeral(8)
    assert(listener > 0)
    assert(net.listener_port(listener) > 0)
    assert(net.close_fd(listener) == 0)
}
