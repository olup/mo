import * as buffer from "std/buffer"
import * as core from "core/unsafe"

fn exercise() -> Int {
    let bytes_buf: buffer.ByteBuffer = buffer.byte_buffer_new(1)
    buffer.byte_buffer_push(bytes_buf, 65)
    buffer.byte_buffer_push(bytes_buf, 66)
    buffer.byte_buffer_set(bytes_buf, 1, 90)
    let text = buffer.byte_buffer_finish(bytes_buf)
    let length = buffer.byte_buffer_length(bytes_buf)
    let first = buffer.byte_buffer_get(bytes_buf, 0)
    let second = buffer.byte_buffer_get(bytes_buf, 1)
    if length == 2 {
        if first == 65 {
            if second == 90 {
                return length
            }
        }
    }
    return 1
}

fn main() -> Int {
    let alloc0 = core.mem_alloc_count()
    let free0 = core.mem_free_count()
    let len = exercise()
    let alloc1 = core.mem_alloc_count()
    let free1 = core.mem_free_count()
    if len != 2 {
        return 1
    }
    if alloc1 < alloc0 + 2 {
        return 2
    }
    if free1 < free0 + 1 {
        return 3
    }
    return 42
}
