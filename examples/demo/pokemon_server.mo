import * as async from "std/async"
import * as async_tcp from "std/async_tcp"
import * as buffer from "std/buffer"
import * as bytes from "std/bytes"
import * as core from "core/unsafe"
import * as event from "std/event"
import * as express from "lib/express"
import * as fs from "std/fs"
import * as http from "std/http"
import * as int from "std/int"
import * as io from "std/io"
import * as map from "std/map"
import * as net from "std/net"
import * as option from "std/option"
import { Option } from "std/option"
import * as path from "std/path"
import * as pokemon from "lib/pokemon"
import * as process from "std/process"
import * as result from "std/result"
import { Result } from "std/result"
import * as server from "lib/pokemon_server"
import * as shared from "std/shared"
import * as slice from "std/slice"
import * as String from "std/string"
import * as vec from "std/vec"

extern "C" {
    fn pipe(fds: Int) -> Int32
}

fn fd_at(fds: Int, slot: Int) -> Int {
    return bytes.load_u32_le(fds, slot * 4)
}

fn request_count(cwd: &Str) -> Int {
    let config = path.join(cwd, "mo_pokemon_server_requests.txt")
    let requested = int.parse_decimal_or(fs.read_text_or(config, "1024"), 1024)
    if requested < 1 {
        return 1024
    }
    return requested
}

fn per_worker_count(total: Int) -> Int {
    return (total + 3) / 4
}

fn served_capacity(total: Int) -> Int {
    let values: vec.Vec<Int> = vec.new<Int>()
    vec.push<Int>(values, per_worker_count(total))
    vec.push<Int>(values, 4)
    return vec.get<Int>(values, 0) * vec.get<Int>(values, 1)
}

fn capacity_digits(total: Int) -> Int {
    let labels: map.Map<String, String> = map.new<String, String>()
    let key = String.from("capacity")
    let value = String.from_int(total)
    map.put<String, String>(labels, key, value)
    return String.len(map.get<String, String>(labels, "capacity"))
}

fn shared_capacity(total: Int) -> Int {
    let current: shared.Shared<Int> = shared.new_int(total)
    let mirror: shared.Shared<Int> = shared.clone_int(current)
    return shared.get_int(mirror)
}

fn buffer_slice_probe(total: Int) -> Int {
    let builder: buffer.StringBuilder = buffer.string_builder_new(8)
    buffer.string_builder_append(builder, "cap:")
    buffer.string_builder_append_int(builder, total)
    let label = buffer.string_builder_finish(builder)
    let label_slice = slice.from_str(label)
    let prefix = slice.subslice(label_slice, 0, 3)

    let marker: buffer.ByteBuffer = buffer.byte_buffer_new(1)
    buffer.byte_buffer_push(marker, 79)
    buffer.byte_buffer_push(marker, 107)
    buffer.byte_buffer_set(marker, 1, 75)
    let marker_text = buffer.byte_buffer_finish(marker)
    let marker_slice = slice.from_str(marker_text)

    if prefix[0] == 99 {
        if prefix[1] == 97 {
            if prefix[2] == 112 {
                if marker_slice[0] == 79 {
                    if marker_slice[1] == 75 {
                        return slice.len(label_slice) + slice.len(marker_slice)
                    }
                }
            }
        }
    }
    return 0
}

fn typed_request_probe() -> Int {
    let fds = core.alloc(8)
    if pipe(fds) != 0 {
        core.free(fds)
        return 0 - 1
    }
    let read_fd = fd_at(fds, 0)
    let write_fd = fd_at(fds, 1)
    io.write_fd(write_fd, "POST /pokemon HTTP/1.1\r\nHost: demo\r\nX-Demo: typed\r\nContent-Length: 7\r\n\r\npayload")
    io.close_fd(write_fd)
    let request = http.read_request(read_fd)
    io.close_fd(read_fd)
    core.free(fds)
    let result = request.route_id + http.request_header_count(request) + String.len(request.body)
    http.request_destroy(request)
    return result
}

fn typed_response_probe() -> Int {
    let mut response = http.created_json_response("{\"ok\":true}")
    http.response_header_put(response, String.from("X-Demo"), String.from("headers"))
    let header = http.response_header(response, "X-Demo")
    let wire = http.render(response)
    let result = response.status + http.response_header_count(response) + String.len(header) + String.len(wire)
    String.free_owned(header)
    String.free_owned(wire)
    http.response_destroy(response)
    return result
}

fn increment(value: Int) -> Int {
    return value + 1
}

fn require_positive(value: Int) -> option.Option<Int> {
    if value > 0 {
        return Some(value)
    }
    return None
}

fn result_positive(value: Int) -> result.Result<Int, Int> {
    if value > 0 {
        return Ok(value)
    }
    return Err(0)
}

fn fallback_option() -> option.Option<Int> {
    return Some(1)
}

fn bump_error(value: Int) -> Int {
    return value + 1
}

fn recover_error(value: Int) -> result.Result<Int, Int> {
    return Ok(value + 41)
}

fn combinator_probe() -> Int {
    let maybe: Option<Int> = Some(41)
    let mapped = option.or_else(option.and_then(option.map(maybe, increment), require_positive), fallback_option)
    let ok: Result<Int, Int> = Ok(option.unwrap_or(mapped, 0))
    let recovered = result.or_else(Err(1), recover_error)
    return result.unwrap_or(result.map_err(result.and_then(result.map(ok, increment), result_positive), bump_error), 0) + result.unwrap_or(recovered, 0)
}

fn storage_try_pipeline(file: &Str) -> Result<Int, pokemon.StoreError> {
    let value = pokemon.train_checked(file)?
    return Ok(value.level)
}

fn storage_try_probe(file: &Str) -> Int {
    return match storage_try_pipeline(file) {
        Ok(level) => level
        Err(error) => 0 - 1
    }
}

async fn async_demo_value() -> Int {
    return 1
}

fn async_executor_probe() -> Int {
    return async.block_on(async_demo_value())
}

fn async_tcp_probe() -> Int {
    let loop = event.new()
    let listener = net.tcp_listener_new(16)
    let port = net.tcp_listener_port(listener)
    if port > 0 {
        let client = net.tcp_stream_connect_loopback(port)
        let server = async_tcp.accept(loop, listener)
        let wrote = async_tcp.write(loop, client, "P")
        if wrote == 1 {
            let byte = async_tcp.read_byte(loop, server)
            if byte == 80 {
                return event.backend(loop)
            }
            return 0 - 4
        }
        return 0 - 3
    }
    return 0 - 1
}

fn main() -> Int {
    let cwd = process.current_dir()
    let file = path.join(cwd, "mo_pokemon_server.json")
    pokemon.reset(file)
    let requests = request_count(cwd)

    let app = server.app(128)
    let capacity = shared_capacity(served_capacity(requests))
    if express.app_port(app) > 0 {
        let port = express.app_port(app)
        if port > 0 {
            io.write_fd(1, "Mo Pokemon API: http://127.0.0.1:")
            io.write_fd(1, String.from_int(port))
            io.write_fd(1, "\n")
            io.write_fd(1, "GET /pokemon, POST /pokemon, GET /health\n")
            io.write_fd(1, "Request capacity: ")
            io.write_fd(1, String.from_int(capacity))
            io.write_fd(1, "\n")
            io.write_fd(1, "Capacity digits: ")
            io.write_fd(1, String.from_int(capacity_digits(capacity)))
            io.write_fd(1, "\n")
            io.write_fd(1, "Buffer/slice probe: ")
            io.write_fd(1, String.from_int(buffer_slice_probe(capacity)))
            io.write_fd(1, "\n")
            io.write_fd(1, "Typed request probe: ")
            io.write_fd(1, String.from_int(typed_request_probe()))
            io.write_fd(1, "\n")
            io.write_fd(1, "Typed response probe: ")
            io.write_fd(1, String.from_int(typed_response_probe()))
            io.write_fd(1, "\n")
            io.write_fd(1, "Registered routes: ")
            io.write_fd(1, String.from_int(express.route_count(app)))
            io.write_fd(1, "\n")
            io.write_fd(1, "Router lookup: method/path\n")
            io.write_fd(1, "Combinator probe: ")
            io.write_fd(1, String.from_int(combinator_probe()))
            io.write_fd(1, "\n")
            io.write_fd(1, "Storage try probe: ")
            io.write_fd(1, String.from_int(storage_try_probe(file)))
            io.write_fd(1, "\n")
            io.write_fd(1, "Async TCP backend: ")
            io.write_fd(1, String.from_int(async_tcp_probe()))
            io.write_fd(1, "\n")
            io.write_fd(1, "Async executor probe: ")
            io.write_fd(1, String.from_int(async_executor_probe()))
            io.write_fd(1, "\n")
            let served = express.serve_async_threadpool4(app, file, per_worker_count(requests))
            express.app_close(app)
            return served
        }
        express.app_close(app)
        return 1
    }
    return 2
}
