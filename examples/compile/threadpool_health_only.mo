import * as express from "lib/express"
import * as fs from "std/fs"
import * as io from "std/io"
import * as net from "std/net"
import * as path from "std/path"
import * as pokemon from "lib/pokemon"
import * as server from "lib/pokemon_server"
import * as process from "std/process"

fn main() -> Int {
    let cwd = process.current_dir()
    let file = path.join(cwd, "mo_threadpool_health_only.json")
    pokemon.reset(file)

    let app = server.app(16)
    if express.app_port(app) > 0 {
        let port = express.app_port(app)
        if port > 0 {
            let client1 = net.tcp_connect_loopback(port)
            let client2 = net.tcp_connect_loopback(port)
            let client3 = net.tcp_connect_loopback(port)
            let client4 = net.tcp_connect_loopback(port)
            io.write_fd(client1, "GET /health HTTP/1.1\r\nHost: localhost\r\n\r\n")
            io.write_fd(client2, "GET /health HTTP/1.1\r\nHost: localhost\r\n\r\n")
            io.write_fd(client3, "GET /health HTTP/1.1\r\nHost: localhost\r\n\r\n")
            io.write_fd(client4, "GET /health HTTP/1.1\r\nHost: localhost\r\n\r\n")
            let served = express.serve_async_threadpool4(app, file, 1)
            let first1 = io.read_byte_fd(client1)
            let first2 = io.read_byte_fd(client2)
            let first3 = io.read_byte_fd(client3)
            let first4 = io.read_byte_fd(client4)
            net.close_fd(client1)
            net.close_fd(client2)
            net.close_fd(client3)
            net.close_fd(client4)
            fs.remove(file)
            express.app_close(app)
            if served == 4 {
                if first1 == 72 {
                    if first2 == 72 {
                        if first3 == 72 {
                            if first4 == 72 {
                                return 42
                            }
                        }
                    }
                }
            }
            return 9
        }
        fs.remove(file)
        express.app_close(app)
        return 11
    }
    fs.remove(file)
    return 10
}
