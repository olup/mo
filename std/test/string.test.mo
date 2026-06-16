import * as bytes from "std/bytes"
import * as String from "std/string"

test "std string module test file pattern" {
    let value = String.concat("mo", String.from_byte(33))
    assert(value == "mo!")
    assert(String.len(value) == 3)
    assert(bytes.string_load8(value, 2) == 33)
}

test "std string read APIs accept Str views" {
    let prefix: Str = "mo"
    let suffix: Str = "!"
    let value = String.concat(prefix, suffix)
    assert(value == "mo!")
    assert(String.len(prefix) == 2)
    assert(bytes.string_load8(prefix, 1) == 111)
}

test "std string clone makes an owned copy" {
    let original = String.from("mo")
    let copied = String.clone(original)
    assert(original == copied)
    assert(String.len(original) == 2)
    assert(String.len(copied) == 2)
}
