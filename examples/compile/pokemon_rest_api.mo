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
    let file = path.join(cwd, "mo_pokemon_rest_api.json")
    pokemon.reset(file)

    let app = server.app(16)
    if express.app_port(app) > 0 {
        let port = express.app_port(app)
        if port > 0 {
            let get_client = net.tcp_connect_loopback(port)
            if get_client > 0 {
                io.write_fd(get_client, "GET /pokemon HTTP/1.1\r\nHost: localhost\r\n\r\n")
                let get_written = express.handle_once(app, file)
                let first = io.read_byte_fd(get_client)
                net.close_fd(get_client)
                if get_written > 0 {
                    if first == 72 {
                        let post_client = net.tcp_connect_loopback(port)
                        if post_client > 0 {
                            io.write_fd(post_client, "POST /pokemon HTTP/1.1\r\nHost: localhost\r\nContent-Length: 0\r\n\r\n")
                            let post_written = express.handle_once(app, file)
                            let post_first = io.read_byte_fd(post_client)
                            net.close_fd(post_client)
                            let value = pokemon.read(file)
                            fs.remove(file)
                            express.app_close(app)
                            if post_written > 0 {
                                if post_first == 72 {
                                    if value.level == 6 {
                                        return 42
                                    }
                                    return 8
                                }
                                return 7
                            }
                            return 6
                        }
                        fs.remove(file)
                        express.app_close(app)
                        return 5
                    }
                    fs.remove(file)
                    express.app_close(app)
                    return 4
                }
                fs.remove(file)
                express.app_close(app)
                return 3
            }
            fs.remove(file)
            express.app_close(app)
            return 2
        }
        fs.remove(file)
        express.app_close(app)
        return 1
    }
    fs.remove(file)
    return 9
}
