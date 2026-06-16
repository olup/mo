import * as core from "core/unsafe"
import * as express from "lib/express"
import * as fs from "std/fs"
import * as io from "std/io"
import * as net from "std/net"
import * as path from "std/path"
import * as pokemon from "lib/pokemon"
import * as process from "std/process"
import * as server from "lib/pokemon_server"

fn main() -> Int {
    let cwd = process.current_dir()
    let file = path.join(cwd, "mo_pokemon_threadpool_memory.json")
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
                            io.write_fd(client2, "GET /health HTTP/1.1\r\nHost: localhost\r\n\r\n")
                            io.write_fd(client3, "GET /health HTTP/1.1\r\nHost: localhost\r\n\r\n")
                            io.write_fd(client4, "GET /health HTTP/1.1\r\nHost: localhost\r\n\r\n")

                            let alloc0 = core.mem_alloc_count()
                            let free0 = core.mem_free_count()
                            let live0 = core.mem_live_bytes()
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
                            let alloc1 = core.mem_alloc_count()
                            let free1 = core.mem_free_count()
                            let live1 = core.mem_live_bytes()
                            let high1 = core.mem_high_water_bytes()
                            if served == 4 {
                                if first1 == 72 {
                                    if first2 == 72 {
                                        if first3 == 72 {
                                            if first4 == 72 {
                                                if alloc1 < alloc0 + 4 {
                                                    return 21
                                                }
                                                if free1 < free0 + 4 {
                                                    return 22
                                                }
                                                if live1 > live0 + 4096 {
                                                    return 23
                                                }
                                                if high1 < live0 {
                                                    return 24
                                                }
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
