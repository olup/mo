import * as core from "core/unsafe"
import * as http from "std/http"
import * as net from "std/net"
import * as String from "std/string"

import * as task from "std/task"
import * as vec from "std/vec"

pub struct App {
    listener: Int
    route_methods: vec.Vec<Int>
    route_paths: vec.Vec<String>
    route_handlers: vec.Vec<Int>
    before_handlers: vec.Vec<fn(Int, &http.Request, &Str) -> Int>
}

pub fn listen(backlog: Int) -> Int {
    return net.tcp_listen_ephemeral(backlog)
}

pub fn port(app: Int) -> Int {
    return net.listener_port(app)
}

pub fn close(app: Int) -> Int {
    return net.close_fd(app)
}

pub fn accept(app: Int) -> Int {
    net.wait_readable(app)
    return net.accept_fd(app)
}

pub fn route(client: Int) -> Int {
    return http.request_route(client)
}

pub fn write_json(client: Int, body: &Str) -> Int {
    return http.write_json(client, body)
}

pub fn write_created_json(client: Int, body: &Str) -> Int {
    return http.write_created_json(client, body)
}

pub fn write_health(client: Int) -> Int {
    return http.write_health(client)
}

pub fn write_not_found(client: Int) -> Int {
    return http.write_not_found(client)
}

pub fn write_response(client: Int, response: &http.Response) -> Int {
    return http.write_response(client, response)
}

pub fn close_client(client: Int) -> Int {
    return net.close_fd(client)
}

pub fn default_handler(client: Int, request: &http.Request, context: &Str) -> http.Response {
    return http.not_found_response()
}

pub fn default_middleware(client: Int, request: &http.Request, context: &Str) -> Int {
    return 0
}

fn get_method_id() -> Int {
    return 1
}

fn post_method_id() -> Int {
    return 2
}

pub fn new() -> App {
    return with_backlog(128)
}

pub fn with_backlog(backlog: Int) -> App {
    return App {
        listener: listen(backlog),
        route_methods: vec.new<Int>(),
        route_paths: vec.new<String>(),
        route_handlers: vec.new<Int>(),
        before_handlers: vec.new<fn(Int, &http.Request, &Str) -> Int>()
    }
}

pub fn clone(app: &App) -> App {
    return App {
        listener: app.listener,
        route_methods: app.route_methods,
        route_paths: app.route_paths,
        route_handlers: app.route_handlers,
        before_handlers: app.before_handlers
    }
}

pub fn app_port(app: &App) -> Int {
    return port(app.listener)
}

pub fn app_close(app: &App) -> Int {
    return close(app.listener)
}

pub fn route_count(app: &App) -> Int {
    return vec.length<Int>(app.route_handlers)
}

pub fn use_before(app: &mut App, handler: fn(Int, &http.Request, &Str) -> Int) -> Int {
    return vec.push<fn(Int, &http.Request, &Str) -> Int>(app.before_handlers, handler)
}

pub fn run_before(app: &App, client: Int, request: &http.Request, context: &Str) -> Int {
    let mut index = 0
    let mut result = 0
    while index < vec.length<fn(Int, &http.Request, &Str) -> Int>(app.before_handlers) {
        let handler: fn(Int, &http.Request, &Str) -> Int = vec.get<fn(Int, &http.Request, &Str) -> Int>(app.before_handlers, index)
        result = handler(client, request, context)
        if result < 0 {
            return result
        }
        index += 1
    }
    return result
}

pub fn get(app: &mut App, path: &Str, handler: fn(Int, &http.Request, &Str) -> http.Response) -> Int {
    vec.push<Int>(app.route_methods, get_method_id())
    vec.push<String>(app.route_paths, String.from(path))
    vec.push<Int>(app.route_handlers, core.function_ptr_response_handler(handler))
    return route_count(app)
}

pub fn post(app: &mut App, path: &Str, handler: fn(Int, &http.Request, &Str) -> http.Response) -> Int {
    vec.push<Int>(app.route_methods, post_method_id())
    vec.push<String>(app.route_paths, String.from(path))
    vec.push<Int>(app.route_handlers, core.function_ptr_response_handler(handler))
    return route_count(app)
}

fn route_index_for_request(app: &App, request: &http.Request) -> Int {
    let mut index = 0
    while index < vec.length<Int>(app.route_methods) {
        let route_path = String.from(vec.get<String>(app.route_paths, index))
        if vec.get<Int>(app.route_methods, index) == request.method && request.path == route_path {
            return index
        }
        index += 1
    }
    return 0 - 1
}

fn handler_at(app: &App, index: Int) -> fn(Int, &http.Request, &Str) -> http.Response {
    if index < 0 {
        return default_handler
    }
    if index >= vec.length<Int>(app.route_handlers) {
        return default_handler
    }
    return core.function_from_ptr_response_handler(vec.get<Int>(app.route_handlers, index))
}

pub fn handle_once(app: &App, context: &Str) -> Int {
    let client = accept(app.listener)
    if client > 0 {
        let request = http.read_request(client)
        let before = run_before(app, client, request, context)
        if before < 0 {
            close_client(client)
            return before
        }
        let route_index = route_index_for_request(app, request)
        if route_index >= 0 {
            let handler: fn(Int, &http.Request, &Str) -> http.Response = handler_at(app, route_index)
            let response: http.Response = handler(client, request, context)
            let written = http.write_response(client, response)
            close_client(client)
            return written
        }
        let written = write_not_found(client)
        close_client(client)
        return written
    }
    return 0 - 1
}

pub fn serve_count(app: &App, context: &Str, count: Int) -> Int {
    let mut served = 0
    while served < count {
        let written = handle_once(app, context)
        if written > 0 {
            served += 1
        }
    }
    return served
}

fn request_job(app: &App, context: &Str) -> Int {
    let job = core.alloc(64)
    core.store64(job, 0, app.listener)
    core.store64(job, 8, vec.data<Int>(app.route_methods))
    core.store64(job, 16, vec.data<String>(app.route_paths))
    core.store64(job, 24, vec.data<Int>(app.route_handlers))
    core.store64(job, 32, vec.length<Int>(app.route_handlers))
    core.store64(job, 40, vec.data<fn(Int, &http.Request, &Str) -> Int>(app.before_handlers))
    core.store64(job, 48, vec.length<fn(Int, &http.Request, &Str) -> Int>(app.before_handlers))
    core.store64(job, 56, core.string_clone_ptr(context))
    return job
}

fn free_request_job(job: Int) {
    core.free(job)
}

fn free_unsubmitted_request_job(job: Int) {
    core.free(core.load64(job, 56))
    core.free(job)
}

fn run_before_job(job: Int, client: Int, request: &http.Request, context: &Str) -> Int {
    let data = core.load64(job, 40)
    let length = core.load64(job, 48)
    let mut index = 0
    let mut result = 0
    while index < length {
        let handler = core.function_from_ptr_request_handler(core.load64(data, index * 8))
        result = handler(client, request, context)
        if result < 0 {
            return result
        }
        index += 1
    }
    return result
}

fn route_index_for_request_job(job: Int, request: &http.Request) -> Int {
    let methods = core.load64(job, 8)
    let paths = core.load64(job, 16)
    let length = core.load64(job, 32)
    let mut index = 0
    while index < length {
        let route_path = String.from(core.string_from_ptr(core.load64(paths, index * 8)))
        if core.load64(methods, index * 8) == request.method && request.path == route_path {
            return index
        }
        index += 1
    }
    return 0 - 1
}

fn handler_at_job(job: Int, index: Int) -> fn(Int, &http.Request, &Str) -> http.Response {
    let handlers = core.load64(job, 24)
    let length = core.load64(job, 32)
    if index < 0 {
        return default_handler
    }
    if index >= length {
        return default_handler
    }
    return core.function_from_ptr_response_handler(core.load64(handlers, index * 8))
}

fn handle_request_job(job: Int) {
    let context_ptr = core.load64(job, 56)
    let context = core.string_from_ptr(context_ptr)
    let client = accept(core.load64(job, 0))
    if client > 0 {
        let request = http.read_request(client)
        let before = run_before_job(job, client, request, context)
        if before < 0 {
            close_client(client)
            free_request_job(job)
            return
        }
        let route_index = route_index_for_request_job(job, request)
        if route_index >= 0 {
            let handler: fn(Int, &http.Request, &Str) -> http.Response = handler_at_job(job, route_index)
            let response: http.Response = handler(client, request, context)
            let written = http.write_response(client, response)
            close_client(client)
            free_request_job(job)
            return
        }
        write_not_found(client)
        close_client(client)
    }
    free_request_job(job)
}

pub fn serve_async_threadpool4(app: &App, context: &Str, per_worker: Int) -> Int {
    let queue = task.queue4_int(handle_request_job)
    let total = per_worker * 4
    let mut submitted = 0
    while submitted < total {
        let job = request_job(app, context)
        let result = task.submit_int(queue, job)
        if result != 0 {
            free_unsubmitted_request_job(job)
            task.close_int(queue)
            task.join_queue_int(queue)
            task.destroy_queue_int(queue)
            return 0 - 1
        }
        submitted += 1
    }

    let closed = task.close_int(queue)
    let joined = task.join_queue_int(queue)
    task.destroy_queue_int(queue)
    if closed == 0 && joined == 0 {
        return total
    }
    return 0 - 1
}
