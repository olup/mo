import * as buffer from "std/buffer"
import * as core from "core/unsafe"

fn exercise() -> Int {
    let out = buffer.new(1)
    buffer.append(out, "abcdef")
    buffer.append_byte(out, 33)
    return buffer.length(out)
}

fn main() -> Int {
    let alloc0 = core.mem_alloc_count()
    let free0 = core.mem_free_count()
    let live0 = core.mem_live_bytes()
    let len = exercise()
    let alloc1 = core.mem_alloc_count()
    let free1 = core.mem_free_count()
    let live1 = core.mem_live_bytes()
    if len != 7 {
        return 1
    }
    if alloc1 < alloc0 + 3 {
        return 2
    }
    if free1 < free0 + 3 {
        return 3
    }
    if live1 > live0 + 16 {
        return 4
    }
    return 42
}
