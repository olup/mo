import * as alloc_buffer from "alloc/buffer"
import * as String from "std/string"

pub struct Buffer {
    data: String
    length_value: Int
    capacity_value: Int
}

pub struct StringBuilder {
    inner: Buffer
}

pub struct ByteBuffer {
    inner: Buffer
}

pub fn new(initial_capacity: Int) -> Buffer {
    let data = alloc_buffer.allocate(initial_capacity)
    return Buffer { data: data, length_value: 0, capacity_value: initial_capacity }
}

pub fn string_builder_new(initial_capacity: Int) -> StringBuilder {
    return StringBuilder { inner: new(initial_capacity) }
}

pub fn length(buffer: &Buffer) -> Int {
    return buffer.length_value
}

pub fn string_builder_length(builder: &StringBuilder) -> Int {
    return length(builder.inner)
}

pub fn capacity(buffer: &Buffer) -> Int {
    return buffer.capacity_value
}

pub fn string_builder_capacity(builder: &StringBuilder) -> Int {
    return capacity(builder.inner)
}

pub fn remaining(buffer: &Buffer) -> Int {
    return buffer.capacity_value - buffer.length_value
}

pub fn string_builder_remaining(builder: &StringBuilder) -> Int {
    return remaining(builder.inner)
}

pub fn byte_buffer_new(initial_capacity: Int) -> ByteBuffer {
    return ByteBuffer { inner: new(initial_capacity) }
}

pub fn byte_buffer_length(bytes: &ByteBuffer) -> Int {
    return length(bytes.inner)
}

pub fn byte_buffer_capacity(bytes: &ByteBuffer) -> Int {
    return capacity(bytes.inner)
}

pub fn byte_buffer_remaining(bytes: &ByteBuffer) -> Int {
    return remaining(bytes.inner)
}

fn grow_to(buffer: &mut Buffer, needed: Int) -> Int {
    if needed <= buffer.capacity_value {
        return buffer.capacity_value
    }
    let mut next_capacity = buffer.capacity_value + buffer.capacity_value + 1
    while next_capacity < needed {
        next_capacity *= 2
    }
    let next = alloc_buffer.allocate(next_capacity)
    let mut index = 0
    while index < buffer.length_value {
        alloc_buffer.store(next, index, alloc_buffer.load(buffer.data, index))
        index += 1
    }
    alloc_buffer.store(next, buffer.length_value, 0)
    alloc_buffer.free(buffer.data)
    buffer.data = next
    buffer.capacity_value = next_capacity
    return next_capacity
}

pub fn append_byte(buffer: &mut Buffer, byte: Int) -> Int {
    if buffer.length_value >= buffer.capacity_value {
        grow_to(buffer, buffer.length_value + 1)
    }
    alloc_buffer.store(buffer.data, buffer.length_value, byte)
    buffer.length_value += 1
    alloc_buffer.store(buffer.data, buffer.length_value, 0)
    return buffer.length_value
}

pub fn load_byte(buffer: &Buffer, index: Int) -> Int {
    if index < 0 {
        return 0 - 1
    }
    if index >= buffer.length_value {
        return 0 - 1
    }
    return alloc_buffer.load(buffer.data, index)
}

pub fn store_byte(buffer: &mut Buffer, index: Int, byte: Int) -> Int {
    if index < 0 {
        return 0 - 1
    }
    if index >= buffer.length_value {
        return 0 - 1
    }
    alloc_buffer.store(buffer.data, index, byte)
    return byte
}

pub fn string_builder_append_byte(builder: &mut StringBuilder, byte: Int) -> Int {
    return append_byte(builder.inner, byte)
}

pub fn byte_buffer_push(bytes: &mut ByteBuffer, byte: Int) -> Int {
    return append_byte(bytes.inner, byte)
}

pub fn byte_buffer_get(bytes: &ByteBuffer, index: Int) -> Int {
    return load_byte(bytes.inner, index)
}

pub fn byte_buffer_set(bytes: &mut ByteBuffer, index: Int, byte: Int) -> Int {
    return store_byte(bytes.inner, index, byte)
}

pub fn append(buffer: &mut Buffer, value: &Str) -> Int {
    let value_len = String.len(value)
    if buffer.length_value + value_len > buffer.capacity_value {
        grow_to(buffer, buffer.length_value + value_len)
    }
    let mut index = 0
    while index < value_len {
        alloc_buffer.store(buffer.data, buffer.length_value + index, alloc_buffer.load(value, index))
        index += 1
    }
    buffer.length_value += value_len
    alloc_buffer.store(buffer.data, buffer.length_value, 0)
    return buffer.length_value
}

pub fn string_builder_append(builder: &mut StringBuilder, value: &Str) -> Int {
    return append(builder.inner, value)
}

pub fn append_int(buffer: &mut Buffer, value: Int) -> Int {
    let text = String.from_int(value)
    let result = append(buffer, text)
    String.free_owned(text)
    return result
}

pub fn string_builder_append_int(builder: &mut StringBuilder, value: Int) -> Int {
    return append_int(builder.inner, value)
}

pub fn finish(buffer: &Buffer) -> String {
    return buffer.data
}

pub fn string_builder_finish(builder: &StringBuilder) -> String {
    return finish(builder.inner)
}

pub fn byte_buffer_finish(bytes: &ByteBuffer) -> String {
    return finish(bytes.inner)
}

pub fn destroy(buffer: &Buffer) {
    alloc_buffer.free(buffer.data)
}

pub fn string_builder_destroy(builder: &StringBuilder) {
    destroy(builder.inner)
}

pub fn byte_buffer_destroy(bytes: &ByteBuffer) {
    destroy(bytes.inner)
}
