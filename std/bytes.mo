import * as core from "core/unsafe"

pub fn is_digit(byte: Int) -> Bool {
    return byte >= 48 && byte <= 57
}

pub fn digit_value(byte: Int) -> Int {
    if byte >= 48 && byte <= 57 {
        return byte - 48
    }
    return 0 - 1
}

pub fn is_alpha(byte: Int) -> Bool {
    if byte >= 65 && byte <= 90 {
        return true
    }
    if byte >= 97 {
        return byte <= 122
    }
    return false
}

pub fn is_space(byte: Int) -> Bool {
    if byte == 32 {
        return true
    }
    if byte == 9 {
        return true
    }
    if byte == 10 {
        return true
    }
    return byte == 13
}

pub fn load8(ptr: Int, offset: Int) -> Int {
    return core.load8(ptr, offset)
}

pub fn store8(ptr: Int, offset: Int, value: Int) {
    core.store8(ptr, offset, value)
}

pub fn string_load8(value: &Str, offset: Int) -> Int {
    return core.string_load8(value, offset)
}

pub fn zero(ptr: Int, count: Int) {
    let mut index = 0
    while index < count {
        core.store8(ptr, index, 0)
        index += 1
    }
}

pub fn copy(dst: Int, src: Int, count: Int) {
    let mut index = 0
    while index < count {
        core.store8(dst, index, core.load8(src, index))
        index += 1
    }
}

pub fn load_u16_be(ptr: Int, offset: Int) -> Int {
    return core.load8(ptr, offset) * 256 + core.load8(ptr, offset + 1)
}

pub fn load_u16_le(ptr: Int, offset: Int) -> Int {
    return core.load8(ptr, offset) + core.load8(ptr, offset + 1) * 256
}

pub fn store_u16_be(ptr: Int, offset: Int, value: Int) {
    core.store8(ptr, offset, value / 256)
    core.store8(ptr, offset + 1, value % 256)
}

pub fn store_u16_le(ptr: Int, offset: Int, value: Int) {
    core.store8(ptr, offset, value % 256)
    core.store8(ptr, offset + 1, value / 256)
}

pub fn load_u32_le(ptr: Int, offset: Int) -> Int {
    return core.load8(ptr, offset) +
        core.load8(ptr, offset + 1) * 256 +
        core.load8(ptr, offset + 2) * 65536 +
        core.load8(ptr, offset + 3) * 16777216
}

pub fn store_u32_le(ptr: Int, offset: Int, value: Int) {
    core.store32le(ptr, offset, value)
}
