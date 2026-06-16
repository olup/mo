import * as express from "lib/express"
import * as http from "std/http"
import * as io from "std/io"
import * as net from "std/net"

fn status_class(client: Int) -> Int {
    io.read_byte_fd(client)
    io.read_byte_fd(client)
    io.read_byte_fd(client)
    io.read_byte_fd(client)
    io.read_byte_fd(client)
    io.read_byte_fd(client)
    io.read_byte_fd(client)
    io.read_byte_fd(client)
    io.read_byte_fd(client)
    return io.read_byte_fd(client)
}

fn custom_handler(client: Int, request: &http.Request, context: &Str) -> http.Response {
    if request.method == 1 {
        if request.path == "/custom" {
            if request.route_id == 0 {
                return http.created_json_response("{\"dynamic\":true}")
            }
        }
    }
    return http.not_found_response()
}

fn main() -> Int {
    let mut app = express.with_backlog(16)
    express.get(app, "/custom", custom_handler)
    if express.route_count(app) != 1 {
        express.app_close(app)
        return 1
    }
    let port = express.app_port(app)
    if port <= 0 {
        express.app_close(app)
        return 2
    }
    let client = net.tcp_connect_loopback(port)
    if client <= 0 {
        express.app_close(app)
        return 3
    }
    io.write_fd(client, "GET /custom HTTP/1.1\r\nHost: localhost\r\n\r\n")
    let written = express.handle_once(app, "ctx")
    let status = status_class(client)
    net.close_fd(client)
    express.app_close(app)
    if written > 0 {
        if status == 50 {
            return 42
        }
        return 5
    }
    return 4
}
