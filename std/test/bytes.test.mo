import * as bytes from "std/bytes"
import * as core from "core/unsafe"

test "std bytes module test file pattern" {
    assert(bytes.is_digit(48))
    assert(bytes.digit_value(57) == 9)
    let ptr = core.alloc(4)
    bytes.store_u32_le(ptr, 0, 305419896)
    assert(bytes.load_u32_le(ptr, 0) == 305419896)
    core.free(ptr)
}
