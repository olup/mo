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
    io.write_fd(write_fd, "POST /pokemon HTTP/1.1\r\nHost: localhost\r\nContent-Length: 12\r\n\r\nhello world!")
    io.close_fd(write_fd)
    let request = http.read_request(read_fd)
    io.close_fd(read_fd)
    core.free(fds)
    if request.method == 2 {
        if request.route_id == 2 {
            if request.content_length == 12 {
                if String.len(request.body) == 12 {
                    if request.body == "hello world!" {
                        http.request_destroy(request)
                        return 42
                    }
                    http.request_destroy(request)
                    return 8
                }
                http.request_destroy(request)
                return 7
            }
            http.request_destroy(request)
            return 6
        }
        http.request_destroy(request)
        return 5
    }
    http.request_destroy(request)
    return 4
}
