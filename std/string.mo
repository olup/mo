import * as alloc_string from "alloc/string"
import * as core from "core/unsafe"

pub fn new(value: &Str) -> String {
    return alloc_string.copy(value)
}

pub fn from(value: &Str) -> String {
    return alloc_string.copy(value)
}

pub fn clone(value: &Str) -> String {
    return alloc_string.copy(value)
}

pub fn concat(a: &Str, b: &Str) -> String {
    return alloc_string.concat(a, b)
}

pub fn len(value: &Str) -> Int {
    return core.strlen(value)
}

pub fn from_int(value: Int) -> String {
    return alloc_string.from_int(value)
}

pub fn from_byte(byte: Int) -> String {
    return alloc_string.from_byte(byte)
}

pub fn free_owned(value: &String) {
    alloc_string.free(value)
}
