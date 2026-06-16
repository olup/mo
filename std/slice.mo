import * as bytes from "std/bytes"
import * as String from "std/string"

pub struct ByteSlice {
    data: Str
    start: Int
    length_value: Int
}

pub fn from_str(value: &Str) -> ByteSlice {
    return ByteSlice { data: value, start: 0, length_value: String.len(value) }
}

pub fn subslice(value: &ByteSlice, start: Int, length_value: Int) -> ByteSlice {
    if start < 0 {
        return ByteSlice { data: value.data, start: value.start, length_value: 0 }
    }
    if length_value < 0 {
        return ByteSlice { data: value.data, start: value.start + start, length_value: 0 }
    }
    if start > value.length_value {
        return ByteSlice { data: value.data, start: value.start + value.length_value, length_value: 0 }
    }
    let remaining = value.length_value - start
    if length_value > remaining {
        return ByteSlice { data: value.data, start: value.start + start, length_value: remaining }
    }
    return ByteSlice { data: value.data, start: value.start + start, length_value: length_value }
}

pub fn len(value: &ByteSlice) -> Int {
    return value.length_value
}

pub fn is_empty(value: &ByteSlice) -> Bool {
    return value.length_value == 0
}

pub fn get(value: &ByteSlice, index: Int) -> Int {
    if index < 0 {
        return 0 - 1
    }
    if index >= value.length_value {
        return 0 - 1
    }
    return bytes.string_load8(value.data, value.start + index)
}
