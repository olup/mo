import * as express from "lib/express"
import * as core from "core/unsafe"
import * as http from "std/http"
import * as io from "std/io"
import * as net from "std/net"
import * as String from "std/string"

extern "C" {
    fn pipe(fds: Int) -> Int32
}

fn fd_at(fds: Int, index: Int) -> Int {
    return core.load64(fds, index * 4)
}

fn before(client: Int, request: &http.Request, context: &Str) -> Int {
    return request.route_id + 5
}

fn after(client: Int, request: &http.Request, context: &Str) -> Int {
    return http.request_header_count(request) + String.len(request.body)
}

fn handler(client: Int, request: &http.Request, context: &Str) -> http.Response {
    if request.route_id == 2 {
        return http.created_json_response("{\"ok\":true}")
    }
    return http.json_response("{\"ok\":true}")
}

fn main() -> Int {
    let mut app = express.with_backlog(16)
    app.use_before(before)
    app.use_before(after)
    app.get("/pokemon", handler)
    app.post("/pokemon", handler)
    app.get("/health", handler)
    let count = express.route_count(app)
    let fds = core.alloc(8)
    pipe(fds)
    let read_fd = fd_at(fds, 0)
    let write_fd = fd_at(fds, 1)
    io.write_fd(write_fd, "POST /pokemon HTTP/1.1\r\nHost: local\r\nX-Test: typed\r\nContent-Length: 4\r\n\r\nbody")
    net.close_fd(write_fd)
    let request = http.read_request(read_fd)
    net.close_fd(read_fd)
    core.free(fds)
    let before_result = express.run_before(app, 1, request, "ctx")
    let response = handler(1, request, "ctx")
    let handler_result = response.status
    express.app_close(app)
    if count == 3 {
        if before_result == 7 {
            if handler_result == 201 {
                return 42
            }
            return 3
        }
        return 2
    }
    return 1
}
