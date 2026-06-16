import * as http from "std/http"
import * as String from "std/string"

fn main() -> Int {
    let mut response = http.created_json_response("{\"ok\":true}")
    http.response_header_put(response, String.from("X-Demo"), String.from("headers"))
    let value = http.response_header(response, "X-Demo")
    let wire = http.render(response)
    let result = http.response_header_count(response) + String.len(value) + String.len(wire)
    String.free_owned(value)
    String.free_owned(wire)
    http.response_destroy(response)
    if result > 80 {
        return 42
    }
    return 1
}
