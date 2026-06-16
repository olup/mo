import * as bytes from "std/bytes"
import * as http from "std/http"
import * as String from "std/string"

fn main() -> Int {
    let response = http.json_response("{\"ok\":true}")
    let wire = http.render_response(response, "{\"ok\":true}", "application/json")
    if response.status == http.status_ok() {
        if String.len("{\"ok\":true}") == 11 {
            if bytes.string_load8(wire, 0) == 72 {
                if String.len(wire) > String.len("{\"ok\":true}") {
                    return 42
                }
                return 4
            }
            return 3
        }
        return 2
    }
    return 1
}
