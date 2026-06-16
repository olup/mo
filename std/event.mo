import * as net from "std/net"

pub struct EventLoop {
    backend: Int
}

pub fn new() -> EventLoop {
    return EventLoop { backend: 1 }
}

pub fn backend(loop: &EventLoop) -> Int {
    return loop.backend
}

pub fn wait_readable_fd(loop: &EventLoop, fd: Int) -> Int {
    if loop.backend == 1 {
        return net.wait_readable(fd)
    }
    return 0 - 1
}

pub fn wait_writable_fd(loop: &EventLoop, fd: Int) -> Int {
    if loop.backend == 1 {
        return net.wait_writable(fd)
    }
    return 0 - 1
}

pub fn wait_listener(loop: &EventLoop, listener: &net.TcpListener) -> Int {
    return wait_readable_fd(loop, net.tcp_listener_fd(listener))
}

pub fn wait_stream(loop: &EventLoop, stream: &net.TcpStream) -> Int {
    return wait_readable_fd(loop, net.tcp_stream_fd(stream))
}

pub fn wait_stream_writable(loop: &EventLoop, stream: &net.TcpStream) -> Int {
    return wait_writable_fd(loop, net.tcp_stream_fd(stream))
}
