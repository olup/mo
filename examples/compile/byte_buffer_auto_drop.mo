import * as buffer from "std/buffer"
import * as core from "core/unsafe"

fn scoped_bytes() -> Int {
    let bytes_buf: buffer.ByteBuffer = buffer.byte_buffer_new(1)
    buffer.byte_buffer_push(bytes_buf, 65)
    buffer.byte_buffer_push(bytes_buf, 66)
    return buffer.byte_buffer_length(bytes_buf)
}

fn main() -> Int {
    let alloc0 = core.mem_alloc_count()
    let free0 = core.mem_free_count()
    let len = scoped_bytes()
    let alloc1 = core.mem_alloc_count()
    let free1 = core.mem_free_count()
    if len != 2 {
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
