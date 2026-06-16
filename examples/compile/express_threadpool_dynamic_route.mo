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
                return http.json_response("{\"dynamic\":true}")
            }
        }
    }
    return http.not_found_response()
}

fn main() -> Int {
    let mut app = express.with_backlog(16)
    express.get(app, "/custom", custom_handler)
    let port = express.app_port(app)
    if port <= 0 {
        express.app_close(app)
        return 1
    }
    let client1 = net.tcp_connect_loopback(port)
    let client2 = net.tcp_connect_loopback(port)
    let client3 = net.tcp_connect_loopback(port)
    let client4 = net.tcp_connect_loopback(port)
    if client1 > 0 {
        if client2 > 0 {
            if client3 > 0 {
                if client4 > 0 {
                    io.write_fd(client1, "GET /custom HTTP/1.1\r\nHost: localhost\r\n\r\n")
                    io.write_fd(client2, "GET /custom HTTP/1.1\r\nHost: localhost\r\n\r\n")
                    io.write_fd(client3, "GET /custom HTTP/1.1\r\nHost: localhost\r\n\r\n")
                    io.write_fd(client4, "GET /custom HTTP/1.1\r\nHost: localhost\r\n\r\n")
                    let served = express.serve_async_threadpool4(app, "ctx", 1)
                    let status1 = status_class(client1)
                    let status2 = status_class(client2)
                    let status3 = status_class(client3)
                    let status4 = status_class(client4)
                    net.close_fd(client1)
                    net.close_fd(client2)
                    net.close_fd(client3)
                    net.close_fd(client4)
                    express.app_close(app)
                    if served != 4 {
                        return 10 + served
                    }
                    if status1 != 50 {
                        return 20 + status1
                    }
                    if status2 != 50 {
                        return 30 + status2
                    }
                    if status3 != 50 {
                        return 40 + status3
                    }
                    if status4 != 50 {
                        return 50 + status4
                    }
                    return 42
                }
            }
        }
    }
    express.app_close(app)
    return 2
}
