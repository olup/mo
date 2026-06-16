import * as bytes from "std/bytes"
import * as json from "../json"
import * as String from "std/string"

test "json encodes object fields" {
    let name = json.field_string("name", "Pika_!\nchu")
    let level = json.field_int("level", 5)
    let body = json.object(json.append_field(name, level))

    assert(String.len(body) == 32)
    assert(bytes.string_load8(body, 0) == 123)
    assert(bytes.string_load8(body, 1) == 34)
    assert(bytes.string_load8(body, 2) == 110)
    assert(bytes.string_load8(body, 3) == 97)
    assert(bytes.string_load8(body, 4) == 109)
    assert(bytes.string_load8(body, 5) == 101)
    assert(bytes.string_load8(body, 6) == 34)
    assert(bytes.string_load8(body, 7) == 58)
    assert(bytes.string_load8(body, 8) == 34)
    assert(bytes.string_load8(body, 13) == 95)
    assert(bytes.string_load8(body, 14) == 33)
    assert(bytes.string_load8(body, 15) == 92)
    assert(bytes.string_load8(body, 16) == 110)
    assert(bytes.string_load8(body, 20) == 34)
    assert(bytes.string_load8(body, 21) == 44)
    assert(bytes.string_load8(body, 29) == 58)
    assert(bytes.string_load8(body, 30) == 53)
    assert(bytes.string_load8(body, 31) == 125)
}

test "json parses fields with fallbacks" {
    let body = json.object(json.append_field(json.field_int("id", 25), json.field_string("kind", "Electric")))

    assert(json.parse_field_int_or(body, "id", 0) == 25)
    assert(json.parse_field_int_or(body, "missing", 7) == 7)
    assert(String.len(json.parse_field_string_or(body, "kind", "")) == 8)
    assert(bytes.string_load8(json.parse_field_string_or(body, "kind", ""), 0) == 69)
    assert(bytes.string_load8(json.parse_field_string_or(body, "kind", ""), 7) == 99)
    assert(String.len(json.parse_field_string_or(body, "missing", "fallback")) == 8)
}
