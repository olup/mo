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
    let file = path.join(cwd, "mo_pokemon_threadpool_rest_api.json")
    pokemon.reset(file)

    let app = server.app(16)
    if express.app_port(app) > 0 {
        let port = express.app_port(app)
        if port > 0 {
            let client1 = net.tcp_connect_loopback(port)
            let client2 = net.tcp_connect_loopback(port)
            let client3 = net.tcp_connect_loopback(port)
            let client4 = net.tcp_connect_loopback(port)
            if client1 > 0 {
                if client2 > 0 {
                    if client3 > 0 {
                        if client4 > 0 {
                            io.write_fd(client1, "GET /health HTTP/1.1\r\nHost: localhost\r\n\r\n")
                            io.write_fd(client2, "GET /pokemon HTTP/1.1\r\nHost: localhost\r\n\r\n")
                            io.write_fd(client3, "GET /health HTTP/1.1\r\nHost: localhost\r\n\r\n")
                            io.write_fd(client4, "GET /pokemon HTTP/1.1\r\nHost: localhost\r\n\r\n")
                            let served = express.serve_async_threadpool4(app, file, 1)
                            io.read_byte_fd(client1)
                            io.read_byte_fd(client1)
                            io.read_byte_fd(client1)
                            io.read_byte_fd(client1)
                            io.read_byte_fd(client1)
                            io.read_byte_fd(client1)
                            io.read_byte_fd(client1)
                            io.read_byte_fd(client1)
                            io.read_byte_fd(client1)
                            let status1 = io.read_byte_fd(client1)
                            io.read_byte_fd(client2)
                            io.read_byte_fd(client2)
                            io.read_byte_fd(client2)
                            io.read_byte_fd(client2)
                            io.read_byte_fd(client2)
                            io.read_byte_fd(client2)
                            io.read_byte_fd(client2)
                            io.read_byte_fd(client2)
                            io.read_byte_fd(client2)
                            let status2 = io.read_byte_fd(client2)
                            io.read_byte_fd(client3)
                            io.read_byte_fd(client3)
                            io.read_byte_fd(client3)
                            io.read_byte_fd(client3)
                            io.read_byte_fd(client3)
                            io.read_byte_fd(client3)
                            io.read_byte_fd(client3)
                            io.read_byte_fd(client3)
                            io.read_byte_fd(client3)
                            let status3 = io.read_byte_fd(client3)
                            io.read_byte_fd(client4)
                            io.read_byte_fd(client4)
                            io.read_byte_fd(client4)
                            io.read_byte_fd(client4)
                            io.read_byte_fd(client4)
                            io.read_byte_fd(client4)
                            io.read_byte_fd(client4)
                            io.read_byte_fd(client4)
                            io.read_byte_fd(client4)
                            let status4 = io.read_byte_fd(client4)
                            net.close_fd(client1)
                            net.close_fd(client2)
                            net.close_fd(client3)
                            net.close_fd(client4)
                            fs.remove(file)
                            express.app_close(app)
                            if served == 4 {
                                if status1 == 50 {
                                    if status2 == 50 {
                                        if status3 == 50 {
                                            if status4 == 50 {
                                                return 42
                                            }
                                        }
                                    }
                                }
                            }
                            return 9
                        }
                    }
                }
            }
            fs.remove(file)
            express.app_close(app)
            return 2
        }
        fs.remove(file)
        express.app_close(app)
        return 11
    }
    fs.remove(file)
    return 10
}
