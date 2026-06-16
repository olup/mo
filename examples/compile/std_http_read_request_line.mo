import * as bytes from "std/bytes"
import * as core from "core/unsafe"
import * as http from "std/http"
import * as io from "std/io"

extern "C" {
    fn pipe(fds: Int) -> Int32
}

fn fd_at(fds: Int, slot: Int) -> Int {
    return bytes.load_u32_le(fds, slot * 4)
}

fn main() -> Int {
    let fds = core.alloc(8)
    if pipe(fds) != 0 {
        core.free(fds)
        return 1
    }
    let read_fd = fd_at(fds, 0)
    let write_fd = fd_at(fds, 1)
    io.write_fd(write_fd, "GET /health HTTP/1.1\r\nHost: localhost\r\nX-Mode: probe\r\n\r\n")
    io.close_fd(write_fd)
    let request = http.read_request(read_fd)
    io.close_fd(read_fd)
    core.free(fds)
    if request.method != 1 {
        http.request_destroy(request)
        return 2
    }
    if request.method_name != "GET" {
        http.request_destroy(request)
        return 3
    }
    if request.path != "/health" {
        http.request_destroy(request)
        return 4
    }
    if request.route_id != 3 {
        http.request_destroy(request)
        return 5
    }
    if http.request_header(request, "Host") != "localhost" {
        http.request_destroy(request)
        return 6
    }
    if http.request_header(request, "X-Mode") != "probe" {
        http.request_destroy(request)
        return 7
    }
    if http.request_header_count(request) != 2 {
        http.request_destroy(request)
        return 8
    }
    http.request_destroy(request)
    return 42
}
