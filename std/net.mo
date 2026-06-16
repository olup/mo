import * as core from "core/unsafe"
import * as bytes from "std/bytes"
pub struct Socket {
    fd: Int
}

pub struct TcpListener {
    fd: Int
}

pub struct TcpStream {
    fd: Int
}

pub struct SocketAddr {
    port: UInt16
}

extern "C" {
    fn socket(domain: Int32, kind: Int32, protocol: Int32) -> Int32
    fn bind(fd: Int32, addr: Int, len: Int32) -> Int32
    fn listen(fd: Int32, backlog: Int32) -> Int32
    fn accept(fd: Int32, addr: Int, addr_len: Int) -> Int32
    fn connect(fd: Int32, addr: Int, len: Int32) -> Int32
    fn getsockname(fd: Int32, addr: Int, addr_len: Int) -> Int32
    fn read(fd: Int32, buffer: &String, count: Int) -> Int
    fn select(nfds: Int32, readfds: Int, writefds: Int, errorfds: Int, timeout: Int) -> Int32
    fn close(fd: Int32) -> Int32
}

pub fn tcp_socket() -> Result<Socket, IOError>
pub fn listen_socket(socket: Socket) -> Result<TcpListener, IOError>

fn zero_sockaddr_in(addr: Int) {
    core.store8(addr, 0, 0)
    core.store8(addr, 1, 0)
    core.store8(addr, 2, 0)
    core.store8(addr, 3, 0)
    core.store8(addr, 4, 0)
    core.store8(addr, 5, 0)
    core.store8(addr, 6, 0)
    core.store8(addr, 7, 0)
    core.store8(addr, 8, 0)
    core.store8(addr, 9, 0)
    core.store8(addr, 10, 0)
    core.store8(addr, 11, 0)
    core.store8(addr, 12, 0)
    core.store8(addr, 13, 0)
    core.store8(addr, 14, 0)
    core.store8(addr, 15, 0)
}

@target(.macos) {
    fn init_sockaddr_in(addr: Int, port: Int) {
        core.store8(addr, 0, 0)
        core.store8(addr, 1, 0)
        core.store8(addr, 2, 0)
        core.store8(addr, 3, 0)
        core.store8(addr, 4, 0)
        core.store8(addr, 5, 0)
        core.store8(addr, 6, 0)
        core.store8(addr, 7, 0)
        core.store8(addr, 8, 0)
        core.store8(addr, 9, 0)
        core.store8(addr, 10, 0)
        core.store8(addr, 11, 0)
        core.store8(addr, 12, 0)
        core.store8(addr, 13, 0)
        core.store8(addr, 14, 0)
        core.store8(addr, 15, 0)
        core.store8(addr, 0, 16)
        core.store8(addr, 1, 2)
        core.store8(addr, 2, port / 256)
        core.store8(addr, 3, port % 256)
    }
}

@target(.linux) {
    fn init_sockaddr_in(addr: Int, port: Int) {
        core.store8(addr, 0, 0)
        core.store8(addr, 1, 0)
        core.store8(addr, 2, 0)
        core.store8(addr, 3, 0)
        core.store8(addr, 4, 0)
        core.store8(addr, 5, 0)
        core.store8(addr, 6, 0)
        core.store8(addr, 7, 0)
        core.store8(addr, 8, 0)
        core.store8(addr, 9, 0)
        core.store8(addr, 10, 0)
        core.store8(addr, 11, 0)
        core.store8(addr, 12, 0)
        core.store8(addr, 13, 0)
        core.store8(addr, 14, 0)
        core.store8(addr, 15, 0)
        core.store8(addr, 0, 2)
        core.store8(addr, 1, 0)
        core.store8(addr, 2, port / 256)
        core.store8(addr, 3, port % 256)
    }
}

pub fn tcp_listen_ephemeral(backlog: Int) -> Int {
    let fd = socket(2, 1, 0)
    if fd > 0 {
        let addr = core.alloc(16)
        init_sockaddr_in(addr, 0)
        let bind_ok = bind(fd, addr, 16)
        core.free(addr)
        if bind_ok == 0 {
            let listen_ok = listen(fd, backlog)
            if listen_ok == 0 {
                return fd
            }
            close(fd)
            return 0 - 2
        }
        close(fd)
        return 0 - 3
    }
    return 0 - 1
}

pub fn close_fd(fd: Int) -> Int {
    return close(fd)
}

pub fn set_nonblocking(fd: Int) -> Int {
    return core.set_nonblocking_fd(fd)
}

pub fn accept_fd(fd: Int) -> Int {
    return accept(fd, 0, 0)
}

pub fn listener_port(fd: Int) -> Int {
    let addr = core.alloc(16)
    let len = core.alloc(4)
    zero_sockaddr_in(addr)
    bytes.store_u32_le(len, 0, 16)
    let ok = getsockname(fd, addr, len)
    if ok == 0 {
        let port = bytes.load_u16_be(addr, 2)
        core.free(addr)
        core.free(len)
        return port
    }
    core.free(addr)
    core.free(len)
    return 0 - 1
}

pub fn tcp_connect_loopback(port: Int) -> Int {
    let fd = socket(2, 1, 0)
    if fd > 0 {
        let addr = core.alloc(16)
        init_sockaddr_in(addr, port)
        core.store8(addr, 4, 127)
        core.store8(addr, 5, 0)
        core.store8(addr, 6, 0)
        core.store8(addr, 7, 1)
        let ok = connect(fd, addr, 16)
        core.free(addr)
        if ok == 0 {
            return fd
        }
        close(fd)
        return 0 - 2
    }
    return 0 - 1
}

fn fd_mask8(bit: Int) -> Int {
    if bit == 0 {
        return 1
    }
    if bit == 1 {
        return 2
    }
    if bit == 2 {
        return 4
    }
    if bit == 3 {
        return 8
    }
    if bit == 4 {
        return 16
    }
    if bit == 5 {
        return 32
    }
    if bit == 6 {
        return 64
    }
    return 128
}

fn zero_fd_set(set: Int) {
    let mut index = 0
    while index < 128 {
        core.store8(set, index, 0)
        index += 1
    }
}

pub fn wait_readable(fd: Int) -> Int {
    let set = core.alloc(128)
    zero_fd_set(set)
    core.store8(set, fd / 8, fd_mask8(fd % 8))
    let ready = select(fd + 1, set, 0, 0, 0)
    core.free(set)
    return ready
}

pub fn wait_writable(fd: Int) -> Int {
    let set = core.alloc(128)
    zero_fd_set(set)
    core.store8(set, fd / 8, fd_mask8(fd % 8))
    let ready = select(fd + 1, 0, set, 0, 0)
    core.free(set)
    return ready
}

pub fn listener_new(backlog: Int) -> Int {
    return tcp_listen_ephemeral(backlog)
}

pub fn tcp_listener_new(backlog: Int) -> TcpListener {
    return TcpListener { fd: tcp_listen_ephemeral(backlog) }
}

pub fn tcp_listener_fd(listener: &TcpListener) -> Int {
    return listener.fd
}

pub fn tcp_listener_port(listener: &TcpListener) -> Int {
    return listener_port(listener.fd)
}

pub fn tcp_listener_accept(listener: &TcpListener) -> TcpStream {
    return TcpStream { fd: accept_fd(listener.fd) }
}

pub fn tcp_stream_from_fd(fd: Int) -> TcpStream {
    return TcpStream { fd: fd }
}

pub fn tcp_listener_close(listener: &TcpListener) -> Int {
    return close_fd(listener.fd)
}

pub fn listener_accept(listener: Int) -> Int {
    return accept_fd(listener)
}

pub fn listener_close(listener: Int) -> Int {
    return close_fd(listener)
}

pub fn stream_connect_loopback(port: Int) -> Int {
    return tcp_connect_loopback(port)
}

pub fn tcp_stream_connect_loopback(port: Int) -> TcpStream {
    return TcpStream { fd: tcp_connect_loopback(port) }
}

pub fn tcp_stream_fd(stream: &TcpStream) -> Int {
    return stream.fd
}

pub fn stream_read_byte(stream: Int) -> Int {
    let buffer = core.alloc_string(1)
    let count = read(stream, buffer, 1)
    if count == 1 {
        return core.string_load8(buffer, 0)
    }
    return 0 - 1
}

pub fn stream_write(stream: Int, text: &Str) -> Int {
    return core.write(stream, text)
}

pub fn tcp_stream_read_byte(stream: &TcpStream) -> Int {
    return stream_read_byte(stream.fd)
}

pub fn tcp_stream_write(stream: &TcpStream, text: &Str) -> Int {
    return stream_write(stream.fd, text)
}

pub fn stream_close(stream: Int) -> Int {
    return close_fd(stream)
}

pub fn tcp_stream_close(stream: &TcpStream) -> Int {
    return close_fd(stream.fd)
}
