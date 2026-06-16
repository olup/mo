import * as event from "std/event"
import * as net from "std/net"

pub fn accept(loop: &event.EventLoop, listener: &net.TcpListener) -> net.TcpStream {
    let ready = event.wait_listener(loop, listener)
    if ready > 0 {
        return net.tcp_listener_accept(listener)
    }
    return net.tcp_stream_from_fd(0 - 1)
}

pub fn read_byte(loop: &event.EventLoop, stream: &net.TcpStream) -> Int {
    let ready = event.wait_stream(loop, stream)
    if ready > 0 {
        return net.tcp_stream_read_byte(stream)
    }
    return 0 - 1
}

pub fn write(loop: &event.EventLoop, stream: &net.TcpStream, text: &Str) -> Int {
    let ready = event.wait_stream_writable(loop, stream)
    if ready > 0 {
        return net.tcp_stream_write(stream, text)
    }
    return 0 - 1
}
