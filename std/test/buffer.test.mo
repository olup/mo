import * as buffer from "std/buffer"
import * as bytes from "std/bytes"


test "std buffer appends strings bytes and ints" {
    let buf = buffer.new(16)
    assert(buffer.length(buf) == 0)
    assert(buffer.capacity(buf) == 16)
    assert(buffer.append(buf, "mo") == 2)
    assert(buffer.append_byte(buf, 33) == 3)
    assert(buffer.append(buf, " #") == 5)
    assert(buffer.append_int(buf, 42) == 7)
    let out = buffer.finish(buf)
    assert(out == "mo! #42")
    assert(bytes.string_load8(out, 0) == 109)
    assert(bytes.string_load8(out, 6) == 50)
    assert(buffer.remaining(buf) == 9)
    buffer.destroy(buf)
}


test "std buffer grows past initial capacity" {
    let buf = buffer.new(3)
    assert(buffer.append(buf, "abc") == 3)
    assert(buffer.append_byte(buf, 33) == 4)
    assert(buffer.capacity(buf) >= 4)
    assert(buffer.finish(buf) == "abc!")
    buffer.destroy(buf)
}


test "std string builder facade appends and finishes text" {
    let builder: buffer.StringBuilder = buffer.string_builder_new(2)
    assert(buffer.string_builder_length(builder) == 0)
    assert(buffer.string_builder_append(builder, "mo") == 2)
    assert(buffer.string_builder_append_byte(builder, 33) == 3)
    assert(buffer.string_builder_append(builder, " #") == 5)
    assert(buffer.string_builder_append_int(builder, 42) == 7)
    assert(buffer.string_builder_capacity(builder) >= 7)
    assert(buffer.string_builder_remaining(builder) >= 0)
    assert(buffer.string_builder_finish(builder) == "mo! #42")
    buffer.string_builder_destroy(builder)
}


test "std byte buffer facade gets sets and grows bytes" {
    let bytes_buf: buffer.ByteBuffer = buffer.byte_buffer_new(1)
    assert(buffer.byte_buffer_length(bytes_buf) == 0)
    assert(buffer.byte_buffer_push(bytes_buf, 65) == 1)
    assert(buffer.byte_buffer_push(bytes_buf, 66) == 2)
    assert(buffer.byte_buffer_get(bytes_buf, 0) == 65)
    assert(buffer.byte_buffer_get(bytes_buf, 1) == 66)
    assert(buffer.byte_buffer_get(bytes_buf, 2) == 0 - 1)
    assert(buffer.byte_buffer_set(bytes_buf, 1, 90) == 90)
    assert(buffer.byte_buffer_get(bytes_buf, 1) == 90)
    assert(buffer.byte_buffer_set(bytes_buf, 2, 33) == 0 - 1)
    assert(buffer.byte_buffer_capacity(bytes_buf) >= 2)
    assert(buffer.byte_buffer_remaining(bytes_buf) >= 0)
    let out = buffer.byte_buffer_finish(bytes_buf)
    assert(out == "AZ")
    assert(bytes.string_load8(out, 0) == 65)
    assert(bytes.string_load8(out, 1) == 90)
    buffer.byte_buffer_destroy(bytes_buf)
}
