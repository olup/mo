import * as core from "core/unsafe"

pub fn copy(value: &Str) -> String {
    return core.string_concat(value, "")
}

pub fn concat(a: &Str, b: &Str) -> String {
    return core.string_concat(a, b)
}

pub fn from_int(value: Int) -> String {
    return core.int_to_string(value)
}

pub fn from_byte(byte: Int) -> String {
    let out = core.alloc_string(2)
    core.string_store8(out, 0, byte)
    core.string_store8(out, 1, 0)
    return out
}

pub fn free(value: &String) {
    core.free(core.string_ptr(value))
}
