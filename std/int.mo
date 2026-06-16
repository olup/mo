import * as bytes from "std/bytes"
import * as String from "std/string"
import * as core from "core/unsafe"

pub fn to_string(value: Int) -> String {
    return String.from_int(value)
}

pub fn digit_value(byte: Int) -> Int {
    return bytes.digit_value(byte)
}

pub fn parse_decimal_or(text: &Str, fallback: Int) -> Int {
    let len = String.len(text)
    if len == 0 {
        return fallback
    }

    let mut index = 0
    let mut sign = 1
    if core.string_load8(text, 0) == 45 {
        sign = 0 - 1
        index = 1
    }

    if index == len {
        return fallback
    }

    let mut value = 0
    while index < len {
        let digit = bytes.digit_value(core.string_load8(text, index))
        if digit < 0 {
            return fallback
        }
        if checked_mul_overflows(value, 10) {
            return fallback
        }
        value = value * 10
        if checked_add_overflows(value, digit) {
            return fallback
        }
        value += digit
        index += 1
    }

    if sign < 0 {
        if value > max_value() {
            return fallback
        }
    }
    return value * sign
}

pub fn parse_decimal(text: &Str) -> Int {
    return parse_decimal_or(text, 0)
}

pub fn checked_add(a: Int, b: Int) -> Int {
    return checked_add_or(a, b, 0)
}

pub fn checked_add_overflows(a: Int, b: Int) -> Bool {
    if b > 0 {
        return a > max_value() - b
    }
    if b < 0 {
        return a < min_value() - b
    }
    return false
}

pub fn checked_add_or(a: Int, b: Int, fallback: Int) -> Int {
    if checked_add_overflows(a, b) {
        return fallback
    }
    return a + b
}

pub fn checked_sub_overflows(a: Int, b: Int) -> Bool {
    if b > 0 {
        return a < min_value() + b
    }
    if b < 0 {
        return a > max_value() + b
    }
    return false
}

pub fn checked_sub_or(a: Int, b: Int, fallback: Int) -> Int {
    if checked_sub_overflows(a, b) {
        return fallback
    }
    return a - b
}

pub fn checked_sub(a: Int, b: Int) -> Int {
    return checked_sub_or(a, b, 0)
}

pub fn checked_mul_overflows(a: Int, b: Int) -> Bool {
    if a == 0 {
        return false
    }
    if b == 0 {
        return false
    }
    if a == min_value() {
        if b == 1 {
            return false
        }
        return true
    }
    if b == min_value() {
        if a == 1 {
            return false
        }
        return true
    }
    let result = a * b
    return result / b != a
}

pub fn checked_mul_or(a: Int, b: Int, fallback: Int) -> Int {
    if checked_mul_overflows(a, b) {
        return fallback
    }
    return a * b
}

pub fn checked_mul(a: Int, b: Int) -> Int {
    return checked_mul_or(a, b, 0)
}

pub fn max_value() -> Int {
    return 9223372036854775807
}

pub fn min_value() -> Int {
    return 0 - 9223372036854775807 - 1
}

pub fn is_i8(value: Int) -> Bool {
    if value < 0 - 128 {
        return false
    }
    return value <= 127
}

pub fn is_u8(value: Int) -> Bool {
    if value < 0 {
        return false
    }
    return value <= 255
}

pub fn is_i16(value: Int) -> Bool {
    if value < 0 - 32768 {
        return false
    }
    return value <= 32767
}

pub fn is_u16(value: Int) -> Bool {
    if value < 0 {
        return false
    }
    return value <= 65535
}

pub fn is_i32(value: Int) -> Bool {
    if value < 0 - 2147483648 {
        return false
    }
    return value <= 2147483647
}

pub fn is_u32(value: Int) -> Bool {
    if value < 0 {
        return false
    }
    return value <= 4294967295
}

pub fn to_i8_or(value: Int, fallback: Int) -> Int {
    if is_i8(value) {
        return value
    }
    return fallback
}

pub fn to_u8_or(value: Int, fallback: Int) -> Int {
    if is_u8(value) {
        return value
    }
    return fallback
}

pub fn to_i16_or(value: Int, fallback: Int) -> Int {
    if is_i16(value) {
        return value
    }
    return fallback
}

pub fn to_u16_or(value: Int, fallback: Int) -> Int {
    if is_u16(value) {
        return value
    }
    return fallback
}

pub fn to_i32_or(value: Int, fallback: Int) -> Int {
    if is_i32(value) {
        return value
    }
    return fallback
}

pub fn to_u32_or(value: Int, fallback: Int) -> Int {
    if is_u32(value) {
        return value
    }
    return fallback
}

pub fn min(a: Int, b: Int) -> Int {
    if a < b {
        return a
    }
    return b
}

pub fn max(a: Int, b: Int) -> Int {
    if a > b {
        return a
    }
    return b
}
