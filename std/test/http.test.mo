import * as bytes from "std/bytes"
import * as http from "std/http"
import * as String from "std/string"

test "std http response builders" {
    let ok = http.response_json("{\"ok\":true}")
    let created = http.response_created_json("{\"ok\":true}")
    assert(String.len(ok) > String.len("{\"ok\":true}"))
    assert(String.len(created) > String.len(ok))
}

test "std http header map stores string headers" {
    let headers = http.headers_new()
    http.headers_put(headers, String.from("Content-Type"), String.from("application/json"))
    http.headers_put(headers, String.from("Content-Length"), String.from("11"))
    http.headers_put(headers, String.from("Content-Type"), String.from("text/plain"))
    assert(http.headers_length(headers) == 3)
    assert(http.headers_get(headers, "Content-Type") == "text/plain")
    assert(http.headers_get(headers, "Content-Length") == "11")
    http.headers_destroy(headers)
}

test "std http renders error status responses" {
    let bad_request = http.bad_request_response()
    let bad_wire = http.render_response(bad_request, "{\"error\":\"bad\"}", "application/json")
    let internal_wire = http.render_response(http.internal_server_error_response(), "{\"error\":\"internal\"}", "application/json")
    assert(bad_request.status == http.status_bad_request())
    assert(String.len(bad_wire) > String.len("{\"error\":\"bad\"}"))
    assert(bytes.string_load8(bad_wire, 9) == 52)
    assert(bytes.string_load8(bad_wire, 10) == 48)
    assert(bytes.string_load8(bad_wire, 11) == 48)
    assert(String.len(internal_wire) > String.len("{\"error\":\"internal\"}"))
    assert(bytes.string_load8(internal_wire, 9) == 53)
    assert(bytes.string_load8(internal_wire, 10) == 48)
    assert(bytes.string_load8(internal_wire, 11) == 48)
    assert(http.internal_server_error_response().status == http.status_internal_server_error())
}

test "std http response stores owned headers" {
    let mut response = http.json_response("{\"ok\":true}")
    http.response_header_put(response, String.from("X-Test"), String.from("headers"))
    http.response_header_put(response, String.from("X-Test"), String.from("latest"))
    let header = http.response_header(response, "X-Test")
    let wire = http.render(response)
    assert(http.response_header_count(response) == 2)
    assert(header == "latest")
    assert(String.len(wire) > String.len("{\"ok\":true}") + String.len("X-Test: latest"))
    String.free_owned(header)
    String.free_owned(wire)
    http.response_destroy(response)
}
