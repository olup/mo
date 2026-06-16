import * as buffer from "std/buffer"
import * as core from "core/unsafe"

fn scoped_builder() -> Int {
    let builder: buffer.StringBuilder = buffer.string_builder_new(2)
    buffer.string_builder_append(builder, "mo")
    buffer.string_builder_append_byte(builder, 33)
    return buffer.string_builder_length(builder)
}

fn main() -> Int {
    let alloc0 = core.mem_alloc_count()
    let free0 = core.mem_free_count()
    let len = scoped_builder()
    let alloc1 = core.mem_alloc_count()
    let free1 = core.mem_free_count()
    if len != 3 {
        return 1
    }
    if alloc1 <= alloc0 {
        return 2
    }
    if free1 < free0 + 1 {
        return 3
    }
    return 42
}
