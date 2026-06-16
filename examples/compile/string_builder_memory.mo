import * as buffer from "std/buffer"
import * as core from "core/unsafe"
import * as String from "std/string"

fn exercise() -> Int {
    let builder: buffer.StringBuilder = buffer.string_builder_new(2)
    buffer.string_builder_append(builder, "mo")
    buffer.string_builder_append_byte(builder, 33)
    buffer.string_builder_append(builder, " #")
    buffer.string_builder_append_int(builder, 42)
    let text = buffer.string_builder_finish(builder)
    return String.len(text)
}

fn main() -> Int {
    let alloc0 = core.mem_alloc_count()
    let free0 = core.mem_free_count()
    let len = exercise()
    let alloc1 = core.mem_alloc_count()
    let free1 = core.mem_free_count()
    if len != 7 {
        return 1
    }
    if alloc1 < alloc0 + 3 {
        return 2
    }
    if free1 < free0 + 3 {
        return 3
    }
    return 42
}
