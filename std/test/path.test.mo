import * as bytes from "std/bytes"
import * as path from "std/path"
import * as String from "std/string"

test "std path module test file pattern" {
    assert(String.len(path.separator()) == 1)
    assert(bytes.string_load8(path.separator(), 0) == 47)
    assert(path.join("a", "b") == "a/b")
}
