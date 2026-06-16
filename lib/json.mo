import * as bytes from "std/bytes"
import * as int from "std/int"
import * as String from "std/string"

pub fn int_value(value: Int) -> String {
    return int.to_string(value)
}

pub fn field_int(name: &Str, value: Int) -> String {
    return String.concat(String.concat(encode_string(name), ":"), int_value(value))
}

pub fn field_string(name: &Str, value: &Str) -> String {
    return String.concat(String.concat(encode_string(name), ":"), encode_string(value))
}

pub fn append_field(fields: &Str, field: &Str) -> String {
    return String.concat(String.concat(fields, ","), field)
}

pub fn object(fields: &Str) -> String {
    return String.concat(String.concat("{", fields), "}")
}

pub fn encode_string(value: &Str) -> String {
    let mut out = String.from("\"")
    let mut index = 0
    let len = String.len(value)
    while index < len {
        let ch = bytes.string_load8(value, index)
        if ch == 34 {
            out = String.concat(out, "\\\"")
        }
        if ch == 92 {
            out = String.concat(out, "\\\\")
        }
        if ch == 10 {
            out = String.concat(out, "\\n")
        }
        if ch != 34 {
            if ch != 92 {
                if ch != 10 {
                    out = String.concat(out, byte_to_string(ch))
                }
            }
        }
        index += 1
    }
    return String.concat(out, "\"")
}

pub fn parse_field_int_or(text: &Str, field: &Str, fallback: Int) -> Int {
    let start = find_field_value(text, field)
    if start < 0 {
        return fallback
    }
    let mut index = start
    let mut out = String.from("")
    let len = String.len(text)
    while index < len {
        let ch = bytes.string_load8(text, index)
        if ch == 45 {
            out = String.concat(out, "-")
        }
        if bytes.is_digit(ch) {
            out = String.concat(out, byte_to_string(ch))
        }
        if ch == 44 {
            return int.parse_decimal_or(out, fallback)
        }
        if ch == 125 {
            return int.parse_decimal_or(out, fallback)
        }
        index += 1
    }
    return int.parse_decimal_or(out, fallback)
}

pub fn parse_field_string_or(text: &Str, field: &Str, fallback: &Str) -> String {
    let start = find_field_value(text, field)
    if start < 0 {
        return String.from(fallback)
    }
    if bytes.string_load8(text, start) != 34 {
        return String.from(fallback)
    }
    let mut index = start + 1
    let mut out = String.from("")
    let len = String.len(text)
    while index < len {
        let ch = bytes.string_load8(text, index)
        if ch == 34 {
            return out
        }
        if ch == 92 {
            let next = bytes.string_load8(text, index + 1)
            if next == 34 {
                out = String.concat(out, "\"")
                index = index + 2
            }
            if next == 92 {
                out = String.concat(out, "\\")
                index = index + 2
            }
            if next == 110 {
                out = String.concat(out, "\n")
                index = index + 2
            }
            if next != 34 {
                if next != 92 {
                    if next != 110 {
                        index += 1
                    }
                }
            }
        }
        if ch != 92 {
            out = String.concat(out, byte_to_string(ch))
            index += 1
        }
    }
    return String.from(fallback)
}

pub fn find_field_value(text: &Str, field: &Str) -> Int {
    let len = String.len(text)
    let field_len = String.len(field)
    let mut index = 0
    while index < len {
        if bytes.string_load8(text, index) == 34 {
            if matches_at(text, index + 1, field) {
                let end_quote = index + field_len + 1
                if bytes.string_load8(text, end_quote) == 34 {
                    let colon = skip_spaces(text, end_quote + 1)
                    if bytes.string_load8(text, colon) == 58 {
                        return skip_spaces(text, colon + 1)
                    }
                }
            }
        }
        index += 1
    }
    return 0 - 1
}

pub fn matches_at(text: &Str, start: Int, pattern: &Str) -> Bool {
    let mut index = 0
    let pattern_len = String.len(pattern)
    while index < pattern_len {
        if bytes.string_load8(text, start + index) != bytes.string_load8(pattern, index) {
            return false
        }
        index += 1
    }
    return true
}

pub fn skip_spaces(text: &Str, start: Int) -> Int {
    let mut index = start
    let len = String.len(text)
    while index < len {
        if !bytes.is_space(bytes.string_load8(text, index)) {
            return index
        }
        index += 1
    }
    return index
}

pub fn byte_to_string(byte: Int) -> String {
    return String.from_byte(byte)
}
