import * as buffer from "std/buffer"
import * as core from "core/unsafe"

fn scoped_buffer() -> Int {
    let out = buffer.new(16)
    buffer.append(out, "mo")
    buffer.append_byte(out, 33)
    return buffer.length(out)
}

fn main() -> Int {
    let alloc0 = core.mem_alloc_count()
    let free0 = core.mem_free_count()
    let len = scoped_buffer()
    let alloc1 = core.mem_alloc_count()
    let free1 = core.mem_free_count()

    if len != 3 {
        return 2
    }
    if alloc1 <= alloc0 {
        return 3
    }
    if free1 < free0 + 1 {
        return 4
    }
    return 42
}
