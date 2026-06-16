import * as alloc_map from "alloc/map"
import * as buffer from "std/buffer"
import * as bytes from "std/bytes"
import * as core from "core/unsafe"
import * as int from "std/int"
import * as String from "std/string"
import * as vec from "std/vec"

pub struct Request {
    pub method: Int
    pub method_name: String
    pub path: String
    pub route_id: Int
    pub content_length: Int
    pub body: String
    header_keys: vec.Vec<String>
    header_values: vec.Vec<String>
}

pub struct Response {
    pub status: Int
    pub body: String
    pub content_type: String
    header_keys: vec.Vec<String>
    header_values: vec.Vec<String>
}

pub struct HeaderMap {
    keys: vec.Vec<String>
    values: vec.Vec<String>
}

pub struct Server {}

extern "C" {
    fn read(fd: Int32, buffer: &String, count: Int) -> Int
}

fn read_byte(fd: Int) -> Int {
    let buffer = core.alloc_string(1)
    let count = read(fd, buffer, 1)
    if count == 1 {
        return core.string_load8(buffer, 0)
    }
    return 0 - 1
}

pub fn request_starts_get(fd: Int) -> Int {
    let g = read_byte(fd)
    let e = read_byte(fd)
    let t = read_byte(fd)
    let space = read_byte(fd)
    if g == 71 {
        if e == 69 {
            if t == 84 {
                if space == 32 {
                    return 1
                }
            }
        }
    }
    return 0
}

fn route_is_pokemon(first: Int, fd: Int) -> Bool {
    if first == 112 {
        let o = read_byte(fd)
        let k = read_byte(fd)
        let e = read_byte(fd)
        let m = read_byte(fd)
        let o2 = read_byte(fd)
        let n = read_byte(fd)
        let end = read_byte(fd)
        if o == 111 {
            if k == 107 {
                if e == 101 {
                    if m == 109 {
                        if o2 == 111 {
                            if n == 110 {
                                if end == 32 {
                                    return true
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    return false
}

fn route_is_health(first: Int, fd: Int) -> Bool {
    if first == 104 {
        let e = read_byte(fd)
        let a = read_byte(fd)
        let l = read_byte(fd)
        let t = read_byte(fd)
        let h = read_byte(fd)
        let end = read_byte(fd)
        if e == 101 {
            if a == 97 {
                if l == 108 {
                    if t == 116 {
                        if h == 104 {
                            if end == 32 {
                                return true
                            }
                        }
                    }
                }
            }
        }
    }
    return false
}

fn parse_route(method: Int, first: Int, fd: Int) -> Int {
    if route_is_pokemon(first, fd) {
        if method == 2 {
            return 2
        }
        return 1
    }
    if method == 1 {
        if route_is_health(first, fd) {
            return 3
        }
    }
    return 0
}

fn read_content_length(fd: Int) -> Int {
    let mut in_headers = false
    let mut reading_number = false
    let mut value = 0
    let mut current_number = 0
    let mut prev3 = 0
    let mut prev2 = 0
    let mut prev1 = 0
    let mut current = read_byte(fd)
    while current >= 0 {
        if in_headers {
            if bytes.is_digit(current) {
                current_number = current_number * 10 + bytes.digit_value(current)
                reading_number = true
            } else {
                if reading_number {
                    value = current_number
                    current_number = 0
                    reading_number = false
                }
            }
        }

        if current == 10 {
            in_headers = true
        }

        if prev3 == 13 {
            if prev2 == 10 {
                if prev1 == 13 {
                    if current == 10 {
                        if reading_number {
                            return current_number
                        }
                        return value
                    }
                }
            }
        }
        prev3 = prev2
        prev2 = prev1
        prev1 = current
        current = read_byte(fd)
    }
    return value
}

fn skip_line(fd: Int) {
    let mut current = read_byte(fd)
    while current >= 0 {
        if current == 10 {
            return
        }
        current = read_byte(fd)
    }
}

fn read_header_block(fd: Int) -> String {
    let out = buffer.new(256)
    let mut prev3 = 0
    let mut prev2 = 0
    let mut prev1 = 0
    let mut current = read_byte(fd)
    while current >= 0 {
        buffer.append_byte(out, current)
        if prev3 == 13 {
            if prev2 == 10 {
                if prev1 == 13 {
                    if current == 10 {
                        return buffer.finish(out)
                    }
                }
            }
        }
        prev3 = prev2
        prev2 = prev1
        prev1 = current
        current = read_byte(fd)
    }
    return buffer.finish(out)
}

fn read_request_line(fd: Int) -> String {
    let out = buffer.new(64)
    let mut current = read_byte(fd)
    while current >= 0 {
        if current == 10 {
            return buffer.finish(out)
        }
        if current != 13 {
            buffer.append_byte(out, current)
        }
        current = read_byte(fd)
    }
    return buffer.finish(out)
}

fn token_from(text: &Str, start: Int) -> String {
    let out = buffer.new(32)
    let len = String.len(text)
    let mut index = start
    while index < len {
        let current = core.string_load8(text, index)
        if current == 32 {
            return buffer.finish(out)
        }
        if current == 13 {
            return buffer.finish(out)
        }
        if current == 10 {
            return buffer.finish(out)
        }
        buffer.append_byte(out, current)
        index += 1
    }
    return buffer.finish(out)
}

fn second_token_from(text: &Str) -> String {
    let len = String.len(text)
    let mut index = 0
    while index < len {
        if core.string_load8(text, index) == 32 {
            return token_from(text, index + 1)
        }
        index += 1
    }
    return String.from("")
}

fn method_id_from(name: &Str) -> Int {
    if name == "GET" {
        return 1
    }
    if name == "POST" {
        return 2
    }
    return 0
}

fn route_id_from(method: Int, path: &Str) -> Int {
    if path == "/pokemon" {
        if method == 2 {
            return 2
        }
        if method == 1 {
            return 1
        }
    }
    if method == 1 {
        if path == "/health" {
            return 3
        }
    }
    return 0
}

fn header_at_host(text: &Str, index: Int, len: Int) -> Bool {
    if index + 5 > len {
        return false
    }
    if core.string_load8(text, index) == 72 {
        if core.string_load8(text, index + 1) == 111 {
            if core.string_load8(text, index + 2) == 115 {
                if core.string_load8(text, index + 3) == 116 {
                    if core.string_load8(text, index + 4) == 58 {
                        return true
                    }
                }
            }
        }
    }
    return false
}

fn header_at_content_type(text: &Str, index: Int, len: Int) -> Bool {
    if index + 13 > len {
        return false
    }
    if core.string_load8(text, index) == 67 {
        if core.string_load8(text, index + 1) == 111 {
            if core.string_load8(text, index + 2) == 110 {
                if core.string_load8(text, index + 3) == 116 {
                    if core.string_load8(text, index + 4) == 101 {
                        if core.string_load8(text, index + 5) == 110 {
                            if core.string_load8(text, index + 6) == 116 {
                                if core.string_load8(text, index + 7) == 45 {
                                    if core.string_load8(text, index + 8) == 84 {
                                        if core.string_load8(text, index + 9) == 121 {
                                            if core.string_load8(text, index + 10) == 112 {
                                                if core.string_load8(text, index + 11) == 101 {
                                                    if core.string_load8(text, index + 12) == 58 {
                                                        return true
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    return false
}

fn header_at_content_length(text: &Str, index: Int, len: Int) -> Bool {
    if index + 15 > len {
        return false
    }
    if core.string_load8(text, index) == 67 {
        if core.string_load8(text, index + 1) == 111 {
            if core.string_load8(text, index + 2) == 110 {
                if core.string_load8(text, index + 3) == 116 {
                    if core.string_load8(text, index + 4) == 101 {
                        if core.string_load8(text, index + 5) == 110 {
                            if core.string_load8(text, index + 6) == 116 {
                                if core.string_load8(text, index + 7) == 45 {
                                    if core.string_load8(text, index + 8) == 76 {
                                        if core.string_load8(text, index + 9) == 101 {
                                            if core.string_load8(text, index + 10) == 110 {
                                                if core.string_load8(text, index + 11) == 103 {
                                                    if core.string_load8(text, index + 12) == 116 {
                                                        if core.string_load8(text, index + 13) == 104 {
                                                            if core.string_load8(text, index + 14) == 58 {
                                                                return true
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    return false
}

fn header_at_x_trace_id(text: &Str, index: Int, len: Int) -> Bool {
    if index + 11 > len {
        return false
    }
    if core.string_load8(text, index) == 88 {
        if core.string_load8(text, index + 1) == 45 {
            if core.string_load8(text, index + 2) == 84 {
                if core.string_load8(text, index + 3) == 114 {
                    if core.string_load8(text, index + 4) == 97 {
                        if core.string_load8(text, index + 5) == 99 {
                            if core.string_load8(text, index + 6) == 101 {
                                if core.string_load8(text, index + 7) == 45 {
                                    if core.string_load8(text, index + 8) == 73 {
                                        if core.string_load8(text, index + 9) == 100 {
                                            if core.string_load8(text, index + 10) == 58 {
                                                return true
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    return false
}

fn header_at_x_mode(text: &Str, index: Int, len: Int) -> Bool {
    if index + 7 > len {
        return false
    }
    if core.string_load8(text, index) == 88 {
        if core.string_load8(text, index + 1) == 45 {
            if core.string_load8(text, index + 2) == 77 {
                if core.string_load8(text, index + 3) == 111 {
                    if core.string_load8(text, index + 4) == 100 {
                        if core.string_load8(text, index + 5) == 101 {
                            if core.string_load8(text, index + 6) == 58 {
                                return true
                            }
                        }
                    }
                }
            }
        }
    }
    return false
}

fn header_value_from(text: &Str, start: Int, len: Int) -> String {
    let out = buffer.new(32)
    let mut index = start
    while index < len {
        if core.string_load8(text, index) != 32 {
            break
        }
        index += 1
    }
    while index < len {
        let current = core.string_load8(text, index)
        if current == 13 {
            return buffer.finish(out)
        }
        if current == 10 {
            return buffer.finish(out)
        }
        buffer.append_byte(out, current)
        index += 1
    }
    return buffer.finish(out)
}

fn header_line_end(text: &Str, start: Int, len: Int) -> Int {
    let mut index = start
    while index < len {
        let current = core.string_load8(text, index)
        if current == 13 {
            return index
        }
        if current == 10 {
            return index
        }
        index += 1
    }
    return len
}

fn header_colon_index(text: &Str, start: Int, end: Int) -> Int {
    let mut index = start
    while index < end {
        if core.string_load8(text, index) == 58 {
            return index
        }
        index += 1
    }
    return 0 - 1
}

fn header_name_from(text: &Str, start: Int, end: Int) -> String {
    let out = buffer.new(32)
    let mut index = start
    while index < end {
        buffer.append_byte(out, core.string_load8(text, index))
        index += 1
    }
    return buffer.finish(out)
}

pub fn read_request(fd: Int) -> Request {
    let request_line = read_request_line(fd)
    let method_name = token_from(request_line, 0)
    let path = second_token_from(request_line)
    let method = method_id_from(method_name)
    let route_id = route_id_from(method, path)
    let raw_headers = read_header_block(fd)
    let header_keys = alloc_map.new_string_keys()
    let header_values = alloc_map.new_string_values()
    let mut content_length = 0
    let len = String.len(raw_headers)
    let mut index = 0
    while index < len {
        let line_end = header_line_end(raw_headers, index, len)
        if line_end != index {
            let colon = header_colon_index(raw_headers, index, line_end)
            if colon > index {
                if header_at_content_length(raw_headers, index, len) {
                    let length_value = header_value_from(raw_headers, colon + 1, line_end)
                    content_length = int.parse_decimal_or(length_value, content_length)
                }
                let name = header_name_from(raw_headers, index, colon)
                let value = header_value_from(raw_headers, colon + 1, line_end)
                alloc_map.put_string_string(header_keys, header_values, name, value)
            }
        }
        index = line_end + 2
    }
    let body = read_body(fd, content_length)
    let request = Request {
        method: method,
        method_name: method_name,
        path: path,
        route_id: route_id,
        content_length: content_length,
        body: body,
        header_keys: header_keys,
        header_values: header_values
    }
    String.free_owned(request_line)
    String.free_owned(raw_headers)
    return request
}

pub fn request_route(fd: Int) -> Int {
    let first = read_byte(fd)
    let second = read_byte(fd)
    let third = read_byte(fd)
    if first == 71 {
        if second == 69 {
            if third == 84 {
                let space = read_byte(fd)
                let slash = read_byte(fd)
                let route_first = read_byte(fd)
                if space == 32 {
                    if slash == 47 {
                        if route_is_pokemon(route_first, fd) {
                            return 1
                        }
                        if route_is_health(route_first, fd) {
                            return 3
                        }
                    }
                }
            }
        }
    }
    if first == 80 {
        if second == 79 {
            if third == 83 {
                let fourth = read_byte(fd)
                let space = read_byte(fd)
                let slash = read_byte(fd)
                let route_first = read_byte(fd)
                if fourth == 84 {
                    if space == 32 {
                        if slash == 47 {
                            if route_is_pokemon(route_first, fd) {
                                return 2
                            }
                        }
                    }
                }
            }
        }
    }
    return 0
}

pub fn request_is_get_pokemon(fd: Int) -> Int {
    if request_route(fd) == 1 {
        return 1
    }
    return 0
}

pub fn request_is_post_pokemon(fd: Int) -> Int {
    if request_route(fd) == 2 {
        return 1
    }
    return 0
}

pub fn write_ok(fd: Int) -> Int {
    return core.write(fd, "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nOK")
}

pub fn status_ok() -> Int {
    return 200
}

pub fn status_created() -> Int {
    return 201
}

pub fn status_bad_request() -> Int {
    return 400
}

pub fn status_not_found() -> Int {
    return 404
}

pub fn status_internal_server_error() -> Int {
    return 500
}

pub fn read_body(fd: Int, content_length: Int) -> String {
    if content_length <= 0 {
        return String.from("")
    }
    let out = buffer.new(content_length + 1)
    let mut index = 0
    while index < content_length {
        buffer.append_byte(out, read_byte(fd))
        index += 1
    }
    return buffer.finish(out)
}

pub fn headers_new() -> HeaderMap {
    return HeaderMap { keys: alloc_map.new_string_keys(), values: alloc_map.new_string_values() }
}

pub fn headers_put(headers: &mut HeaderMap, name: String, value: String) -> Int {
    return alloc_map.put_string_string(headers.keys, headers.values, name, value)
}

pub fn headers_get(headers: &HeaderMap, name: &Str) -> String {
    return alloc_map.get_string_string(headers.keys, headers.values, name)
}

pub fn headers_length(headers: &HeaderMap) -> Int {
    return alloc_map.length_string_string(headers.keys)
}

pub fn headers_destroy(headers: &HeaderMap) {
    alloc_map.destroy_string_string(headers.keys, headers.values)
}

pub fn request_header_count(request: &Request) -> Int {
    return alloc_map.length_string_string(request.header_keys)
}

pub fn request_header(request: &Request, name: &Str) -> String {
    return alloc_map.get_string_string(request.header_keys, request.header_values, name)
}

pub fn request_destroy(request: &Request) {
    String.free_owned(request.method_name)
    String.free_owned(request.path)
    String.free_owned(request.body)
    alloc_map.destroy_string_string(request.header_keys, request.header_values)
}

pub fn response_destroy(response: &Response) {
    String.free_owned(response.body)
    String.free_owned(response.content_type)
    alloc_map.destroy_string_string(response.header_keys, response.header_values)
}

pub fn make_response(status: Int) -> Response {
    return Response {
        status: status,
        body: String.from(""),
        content_type: String.from("text/plain"),
        header_keys: alloc_map.new_string_keys(),
        header_values: alloc_map.new_string_values()
    }
}

pub fn json_response(body: &Str) -> Response {
    return Response {
        status: status_ok(),
        body: String.from(body),
        content_type: String.from("application/json"),
        header_keys: alloc_map.new_string_keys(),
        header_values: alloc_map.new_string_values()
    }
}

pub fn created_json_response(body: &Str) -> Response {
    return Response {
        status: status_created(),
        body: String.from(body),
        content_type: String.from("application/json"),
        header_keys: alloc_map.new_string_keys(),
        header_values: alloc_map.new_string_values()
    }
}

pub fn not_found_response() -> Response {
    return Response {
        status: status_not_found(),
        body: String.from("Not Found"),
        content_type: String.from("text/plain"),
        header_keys: alloc_map.new_string_keys(),
        header_values: alloc_map.new_string_values()
    }
}

pub fn bad_request_response() -> Response {
    return Response {
        status: status_bad_request(),
        body: String.from("Bad Request"),
        content_type: String.from("text/plain"),
        header_keys: alloc_map.new_string_keys(),
        header_values: alloc_map.new_string_values()
    }
}

pub fn internal_server_error_response() -> Response {
    return Response {
        status: status_internal_server_error(),
        body: String.from("Internal Server Error"),
        content_type: String.from("text/plain"),
        header_keys: alloc_map.new_string_keys(),
        header_values: alloc_map.new_string_values()
    }
}

pub fn response_header_put(response: &mut Response, name: String, value: String) -> Int {
    return alloc_map.put_string_string(response.header_keys, response.header_values, name, value)
}

pub fn response_header(response: &Response, name: &Str) -> String {
    return alloc_map.get_string_string(response.header_keys, response.header_values, name)
}

pub fn response_header_count(response: &Response) -> Int {
    return alloc_map.length_string_string(response.header_keys)
}

fn append_response_headers(out: &mut buffer.Buffer, response: &Response) {
    let mut index = 0
    while index < vec.length<String>(response.header_keys) {
        let name = String.from(vec.get<String>(response.header_keys, index))
        let value = String.from(vec.get<String>(response.header_values, index))
        buffer.append(out, name)
        buffer.append(out, ": ")
        buffer.append(out, value)
        buffer.append(out, "\r\n")
        index += 1
    }
}

pub fn render_response(response: &Response, body: &Str, content_type: &Str) -> String {
    let body_len = String.len(body)
    let out = buffer.new(body_len + 128)
    if response.status == status_created() {
        buffer.append(out, "HTTP/1.1 201 Created\r\n")
    }
    if response.status == status_bad_request() {
        buffer.append(out, "HTTP/1.1 400 Bad Request\r\n")
    }
    if response.status == status_not_found() {
        buffer.append(out, "HTTP/1.1 404 Not Found\r\n")
    }
    if response.status == status_internal_server_error() {
        buffer.append(out, "HTTP/1.1 500 Internal Server Error\r\n")
    }
    if response.status != status_created() {
        if response.status != status_bad_request() {
            if response.status != status_not_found() {
                if response.status != status_internal_server_error() {
                    buffer.append(out, "HTTP/1.1 200 OK\r\n")
                }
            }
        }
    }
    buffer.append(out, "Content-Type: ")
    buffer.append(out, content_type)
    buffer.append(out, "\r\nContent-Length: ")
    buffer.append_int(out, body_len)
    buffer.append(out, "\r\n")
    append_response_headers(out, response)
    buffer.append(out, "Connection: close\r\n\r\n")
    buffer.append(out, body)
    return buffer.finish(out)
}

pub fn render(response: &Response) -> String {
    return render_response(response, response.body, response.content_type)
}

pub fn write_response(fd: Int, response: &Response) -> Int {
    let wire = render(response)
    let written = core.write(fd, wire)
    String.free_owned(wire)
    return written
}

pub fn response_json(body: &Str) -> String {
    let body_len = String.len(body)
    let out = buffer.new(body_len + 128)
    buffer.append(out, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: ")
    buffer.append_int(out, body_len)
    buffer.append(out, "\r\nConnection: close\r\n\r\n")
    buffer.append(out, body)
    return buffer.finish(out)
}

pub fn response_created_json(body: &Str) -> String {
    let body_len = String.len(body)
    let out = buffer.new(body_len + 128)
    buffer.append(out, "HTTP/1.1 201 Created\r\nContent-Type: application/json\r\nContent-Length: ")
    buffer.append_int(out, body_len)
    buffer.append(out, "\r\nConnection: close\r\n\r\n")
    buffer.append(out, body)
    return buffer.finish(out)
}

pub fn write_json(fd: Int, body: &Str) -> Int {
    return core.write(fd, response_json(body))
}

pub fn write_created_json(fd: Int, body: &Str) -> Int {
    return core.write(fd, response_created_json(body))
}

pub fn write_not_found(fd: Int) -> Int {
    return core.write(fd, "HTTP/1.1 404 Not Found\r\nContent-Length: 9\r\nConnection: close\r\n\r\nNot Found")
}

pub fn write_health(fd: Int) -> Int {
    return write_json(fd, "{\"status\":\"ok\"}")
}
