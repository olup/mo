import * as bytes from "std/bytes"
import * as core from "core/unsafe"
import * as http from "std/http"
import * as io from "std/io"
import * as String from "std/string"

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
    io.write_fd(write_fd, "POST /pokemon HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nX-Custom-Request-Id: custom-7\r\nX-Feature-Flag: enabled\r\nX-Trace-Id: old\r\nX-Trace-Id: abc123\r\nContent-Length: 12\r\n\r\nhello world!")
    io.close_fd(write_fd)
    let request = http.read_request(read_fd)
    if request.method != 2 {
        io.close_fd(read_fd)
        core.free(fds)
        http.request_destroy(request)
        return 2
    }
    if request.method_name != "POST" {
        io.close_fd(read_fd)
        core.free(fds)
        http.request_destroy(request)
        return 9
    }
    if request.path != "/pokemon" {
        io.close_fd(read_fd)
        core.free(fds)
        http.request_destroy(request)
        return 10
    }
    if request.route_id != 2 {
        io.close_fd(read_fd)
        core.free(fds)
        http.request_destroy(request)
        return 3
    }
    if request.content_length != 12 {
        io.close_fd(read_fd)
        core.free(fds)
        http.request_destroy(request)
        return 7
    }
    if http.request_header_count(request) != 7 {
        let count = http.request_header_count(request)
        io.close_fd(read_fd)
        core.free(fds)
        http.request_destroy(request)
        return 20 + count
    }
    let host = http.request_header(request, "Host")
    let content_type = http.request_header(request, "Content-Type")
    let custom_id = http.request_header(request, "X-Custom-Request-Id")
    let feature_flag = http.request_header(request, "X-Feature-Flag")
    let trace_id = http.request_header(request, "X-Trace-Id")
    if host != "localhost" {
        io.close_fd(read_fd)
        core.free(fds)
        http.request_destroy(request)
        return 5
    }
    if content_type != "application/json" {
        io.close_fd(read_fd)
        core.free(fds)
        http.request_destroy(request)
        return 6
    }
    if custom_id != "custom-7" {
        io.close_fd(read_fd)
        core.free(fds)
        http.request_destroy(request)
        return 12
    }
    if feature_flag != "enabled" {
        io.close_fd(read_fd)
        core.free(fds)
        http.request_destroy(request)
        return 13
    }
    if trace_id != "abc123" {
        io.close_fd(read_fd)
        core.free(fds)
        http.request_destroy(request)
        return 11
    }
    io.close_fd(read_fd)
    core.free(fds)
    if String.len(request.body) != 12 {
        http.request_destroy(request)
        return 8
    }
    if request.body != "hello world!" {
        http.request_destroy(request)
        return 14
    }
    http.request_destroy(request)
    return 42
}
