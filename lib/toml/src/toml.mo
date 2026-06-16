import * as bytes from "std/bytes"
import * as buffer from "std/buffer"
import * as int from "std/int"
import * as String from "std/string"

pub struct ParseError {
    pub line: Int
    pub message: String
}

pub struct ParseResult {
    pub ok: Bool
    pub error: ParseError
}

pub fn parse(text: &Str) -> ParseResult {
    let mut index = 0
    let mut line_number = 1
    let len = String.len(text)
    while index < len {
        let end = logical_entry_end(text, index)
        if end == index {
            line_number += 1
            index = logical_entry_next(text, end)
            continue
        }
        let comment_end = comment_end_in_range(text, index, end)
        let start = trim_start_range(text, index, comment_end)
        let stop = trim_end_range(text, start, comment_end)
        if stop > start {
            if is_section_range(text, start, stop) {
                if stop - start <= 2 {
                    return ParseResult { ok: false, error: ParseError { line: line_number, message: String.from("invalid section") } }
                }
            } else {
                let eq = find_byte_range(text, start, stop, 61)
                if eq <= start {
                    return ParseResult { ok: false, error: ParseError { line: line_number, message: String.from("expected key/value") } }
                }
                let key_start = trim_start_range(text, start, eq)
                let key_end = trim_end_range(text, key_start, eq)
                if key_end <= key_start {
                    return ParseResult { ok: false, error: ParseError { line: line_number, message: String.from("empty key") } }
                }
                let value_start = trim_start_range(text, eq + 1, stop)
                let value_end = trim_end_range(text, value_start, stop)
                if !valid_value_range(text, value_start, value_end) {
                    return ParseResult { ok: false, error: ParseError { line: line_number, message: String.from("invalid value") } }
                }
            }
        }
        line_number += count_newlines(text, index, end) + 1
        index = logical_entry_next(text, end)
    }
    return ParseResult { ok: true, error: ParseError { line: 0, message: String.from("") } }
}

pub fn has(text: &Str, key: &Str) -> Bool {
    let mut index = 0
    let len = String.len(text)
    if len == 0 {
        return false
    }
    while index < len {
        let end = logical_entry_end(text, index)
        if end == index {
            index = logical_entry_next(text, end)
            continue
        }
        let comment_end = comment_end_in_range(text, index, end)
        let start = trim_start_range(text, index, comment_end)
        let stop = trim_end_range(text, start, comment_end)
        if stop > start {
            if !is_section_range(text, start, stop) {
                let eq = find_byte_range(text, start, stop, 61)
                if eq > start {
                    let key_start = trim_start_range(text, start, eq)
                    let key_end = trim_end_range(text, key_start, eq)
                    if key_suffix_matches(text, key_start, key_end, key) {
                        return true
                    }
                }
            }
        }
        index = logical_entry_next(text, end)
    }
    return false
}

pub fn get_string_or(text: &Str, key: &Str, fallback: &Str) -> String {
    let value = get_string(text, key)
    if String.len(value) > 0 {
        return value
    }
    return String.from(fallback)
}

pub fn get_string(text: &Str, key: &Str) -> String {
    let value = find_value(text, key)
    if is_quoted(value) {
        return parse_string(value)
    }
    return String.from("")
}

pub fn get_int_or(text: &Str, key: &Str, fallback: Int) -> Int {
    let value = get_int(text, key)
    if value != 0 {
        return value
    }
    return fallback
}

pub fn get_int(text: &Str, key: &Str) -> Int {
    let mut entry = 0
    let len = String.len(text)
    while entry < len {
        let entry_end = logical_entry_end(text, entry)
        let comment_end = comment_end_in_range(text, entry, entry_end)
        let start = trim_start_range(text, entry, comment_end)
        let stop = trim_end_range(text, start, comment_end)
        if stop > start {
            if !is_section_range(text, start, stop) {
                let eq = find_byte_range(text, start, stop, 61)
                if eq > start {
                    let key_start = trim_start_range(text, start, eq)
                    let key_end = trim_end_range(text, key_start, eq)
                    if key_suffix_matches(text, key_start, key_end, key) {
                        let value_start = trim_start_range(text, eq + 1, stop)
                        let value_end = trim_end_range(text, value_start, stop)
                        if is_int_range(text, value_start, value_end) {
                            return int.parse_decimal_or(substring(text, value_start, value_end), 0)
                        }
                    }
                }
            }
        }
        entry = logical_entry_next(text, entry_end)
    }
    return 0
}

pub fn get_bool_or(text: &Str, key: &Str, fallback: Bool) -> Bool {
    if get_bool(text, key) {
        return true
    }
    return fallback
}

pub fn get_bool(text: &Str, key: &Str) -> Bool {
    let mut entry = 0
    let len = String.len(text)
    while entry < len {
        let entry_end = logical_entry_end(text, entry)
        let comment_end = comment_end_in_range(text, entry, entry_end)
        let start = trim_start_range(text, entry, comment_end)
        let stop = trim_end_range(text, start, comment_end)
        if stop > start {
            if !is_section_range(text, start, stop) {
                let eq = find_byte_range(text, start, stop, 61)
                if eq > start {
                    let key_start = trim_start_range(text, start, eq)
                    let key_end = trim_end_range(text, key_start, eq)
                    if key_suffix_matches(text, key_start, key_end, key) {
                        let value_start = trim_start_range(text, eq + 1, stop)
                        let value_end = trim_end_range(text, value_start, stop)
                        if range_equals(text, value_start, value_end, "true") {
                            return true
                        }
                        if range_equals(text, value_start, value_end, "false") {
                            return false
                        }
                    }
                }
            }
        }
        entry = logical_entry_next(text, entry_end)
    }
    return false
}

pub fn array_len(text: &Str, key: &Str) -> Int {
    let mut entry = 0
    let len = String.len(text)
    while entry < len {
        let entry_end = logical_entry_end(text, entry)
        let comment_end = comment_end_in_range(text, entry, entry_end)
        let start = trim_start_range(text, entry, comment_end)
        let stop = trim_end_range(text, start, comment_end)
        if stop > start {
            if !is_section_range(text, start, stop) {
                let eq = find_byte_range(text, start, stop, 61)
                if eq > start {
                    let key_start = trim_start_range(text, start, eq)
                    let key_end = trim_end_range(text, key_start, eq)
                    if key_suffix_matches(text, key_start, key_end, key) {
                        let value_start = trim_start_range(text, eq + 1, stop)
                        let value_end = trim_end_range(text, value_start, stop)
                        let array_end = array_value_end(text, value_start, value_end)
                        return array_len_range(text, value_start, array_end)
                    }
                }
            }
        }
        entry = logical_entry_next(text, entry_end)
    }
    return 0
}

pub fn array_string_or(text: &Str, key: &Str, item_index: Int, fallback: &Str) -> String {
    let value = array_string(text, key, item_index)
    if String.len(value) > 0 {
        return value
    }
    return String.from(fallback)
}

pub fn array_string(text: &Str, key: &Str, item_index: Int) -> String {
    let mut entry = 0
    let len = String.len(text)
    while entry < len {
        let entry_end = logical_entry_end(text, entry)
        let comment_end = comment_end_in_range(text, entry, entry_end)
        let start = trim_start_range(text, entry, comment_end)
        let stop = trim_end_range(text, start, comment_end)
        if stop > start {
            if !is_section_range(text, start, stop) {
                let eq = find_byte_range(text, start, stop, 61)
                if eq > start {
                    let key_start = trim_start_range(text, start, eq)
                    let key_end = trim_end_range(text, key_start, eq)
                    if key_suffix_matches(text, key_start, key_end, key) {
                        let value_start = trim_start_range(text, eq + 1, stop)
                        let value_end = trim_end_range(text, value_start, stop)
                        let array_end = array_value_end(text, value_start, value_end)
                        let item_start = array_item_start_in_value(text, value_start, array_end, item_index)
                        if item_start < 0 {
                            return String.from("")
                        }
                        let item_end = array_item_end_in_value(text, value_start, array_end, item_index)
                        if is_quoted_range(text, item_start, item_end) {
                            return parse_string_range(text, item_start, item_end)
                        }
                    }
                }
            }
        }
        entry = logical_entry_next(text, entry_end)
    }
    return String.from("")
}

pub fn array_int_or(text: &Str, key: &Str, item_index: Int, fallback: Int) -> Int {
    let value = array_int(text, key, item_index)
    if value != 0 {
        return value
    }
    return fallback
}

pub fn array_int(text: &Str, key: &Str, item_index: Int) -> Int {
    let mut entry = 0
    let len = String.len(text)
    while entry < len {
        let entry_end = logical_entry_end(text, entry)
        let comment_end = comment_end_in_range(text, entry, entry_end)
        let start = trim_start_range(text, entry, comment_end)
        let stop = trim_end_range(text, start, comment_end)
        if stop > start {
            if !is_section_range(text, start, stop) {
                let eq = find_byte_range(text, start, stop, 61)
                if eq > start {
                    let key_start = trim_start_range(text, start, eq)
                    let key_end = trim_end_range(text, key_start, eq)
                    if key_suffix_matches(text, key_start, key_end, key) {
                        let value_start = trim_start_range(text, eq + 1, stop)
                        let value_end = trim_end_range(text, value_start, stop)
                        let array_end = array_value_end(text, value_start, value_end)
                        let item_start = array_item_start_in_value(text, value_start, array_end, item_index)
                        if item_start < 0 {
                            return 0
                        }
                        let item_end = array_item_end_in_value(text, value_start, array_end, item_index)
                        if is_int_range(text, item_start, item_end) {
                            return int.parse_decimal_or(substring(text, item_start, item_end), 0)
                        }
                    }
                }
            }
        }
        entry = logical_entry_next(text, entry_end)
    }
    return 0
}

pub fn array_bool_or(text: &Str, key: &Str, item_index: Int, fallback: Bool) -> Bool {
    if array_bool(text, key, item_index) {
        return true
    }
    return fallback
}

pub fn array_bool(text: &Str, key: &Str, item_index: Int) -> Bool {
    let mut entry = 0
    let len = String.len(text)
    while entry < len {
        let entry_end = logical_entry_end(text, entry)
        let comment_end = comment_end_in_range(text, entry, entry_end)
        let start = trim_start_range(text, entry, comment_end)
        let stop = trim_end_range(text, start, comment_end)
        if stop > start {
            if !is_section_range(text, start, stop) {
                let eq = find_byte_range(text, start, stop, 61)
                if eq > start {
                    let key_start = trim_start_range(text, start, eq)
                    let key_end = trim_end_range(text, key_start, eq)
                    if key_suffix_matches(text, key_start, key_end, key) {
                        let value_start = trim_start_range(text, eq + 1, stop)
                        let value_end = trim_end_range(text, value_start, stop)
                        let array_end = array_value_end(text, value_start, value_end)
                        let item_start = array_item_start_in_value(text, value_start, array_end, item_index)
                        if item_start < 0 {
                            return false
                        }
                        let item_end = array_item_end_in_value(text, value_start, array_end, item_index)
                        if range_equals(text, item_start, item_end, "true") {
                            return true
                        }
                        return false
                    }
                }
            }
        }
        entry = logical_entry_next(text, entry_end)
    }
    return false
}

fn find_value(text: &Str, wanted: &Str) -> String {
    let mut section = String.from("")
    let mut index = 0
    let mut line_number = 1
    let len = String.len(text)
    while index < len {
        let end = logical_line_end(text, index)
        if end == index {
            line_number += 1
            index = logical_line_next(text, end)
            continue
        }
        let raw = line_without_comment(text, index, end)
        let cleaned = trim(raw)
        if String.len(cleaned) > 0 {
            if is_section(cleaned) {
                section = section_name(cleaned)
            } else {
                let eq = find_byte(cleaned, 61)
                if eq > 0 {
                    let local_key = trim(substring(cleaned, 0, eq))
                    if key_matches(section, local_key, wanted) {
                        return trim(substring(cleaned, eq + 1, String.len(cleaned)))
                    }
                }
            }
        }
        line_number += count_newlines(text, index, end) + 1
        index = logical_line_next(text, end)
    }
    return String.from("")
}

fn find_value_start(text: &Str, wanted: &Str) -> Int {
    let mut index = 0
    let len = String.len(text)
    while index < len {
        let end = logical_entry_end(text, index)
        if end == index {
            index = logical_entry_next(text, end)
            continue
        }
        let comment_end = comment_end_in_range(text, index, end)
        let start = trim_start_range(text, index, comment_end)
        let stop = trim_end_range(text, start, comment_end)
        if stop > start {
            if !is_section_range(text, start, stop) {
                let eq = find_byte_range(text, start, stop, 61)
                if eq > start {
                    let key_start = trim_start_range(text, start, eq)
                    let key_end = trim_end_range(text, key_start, eq)
                    if key_suffix_matches(text, key_start, key_end, wanted) {
                        return trim_start_range(text, eq + 1, stop)
                    }
                }
            }
        }
        index = logical_entry_next(text, end)
    }
    return 0 - 1
}

fn find_value_end(text: &Str, wanted: &Str) -> Int {
    let mut index = 0
    let len = String.len(text)
    while index < len {
        let end = logical_entry_end(text, index)
        if end == index {
            index = logical_entry_next(text, end)
            continue
        }
        let comment_end = comment_end_in_range(text, index, end)
        let start = trim_start_range(text, index, comment_end)
        let stop = trim_end_range(text, start, comment_end)
        if stop > start {
            if !is_section_range(text, start, stop) {
                let eq = find_byte_range(text, start, stop, 61)
                if eq > start {
                    let key_start = trim_start_range(text, start, eq)
                    let key_end = trim_end_range(text, key_start, eq)
                    if key_suffix_matches(text, key_start, key_end, wanted) {
                        let value_start = trim_start_range(text, eq + 1, stop)
                        return trim_end_range(text, value_start, stop)
                    }
                }
            }
        }
        index = logical_entry_next(text, end)
    }
    return 0 - 1
}

fn logical_entry_end(text: &Str, start: Int) -> Int {
    let mut index = start
    let mut seen_equals = 0
    let mut bracket_depth = 0
    let mut in_string = false
    let mut escaped = false
    let len = String.len(text)
    while index < len {
        let ch = bytes.string_load8(text, index)
        if is_newline(ch) {
            if seen_equals != 0 {
                if bracket_depth > 0 {
                    index += 1
                    continue
                }
            }
            return index
        }
        if in_string {
            if escaped {
                escaped = false
            } else {
                if ch == 92 {
                    escaped = true
                }
                if ch == 34 {
                    in_string = false
                }
            }
        } else {
            if ch == 34 {
                in_string = true
            }
            if ch == 61 {
                seen_equals = 1
            }
            if seen_equals != 0 {
                if ch == 91 {
                    bracket_depth += 1
                }
            }
            if seen_equals != 0 {
                if ch == 93 {
                    bracket_depth -= 1
                }
            }
        }
        index += 1
    }
    return index
}

fn logical_entry_next(text: &Str, end: Int) -> Int {
    if end < String.len(text) {
        if is_newline(bytes.string_load8(text, end)) {
            return end + 1
        }
    }
    return end
}

fn comment_end_in_range(text: &Str, start: Int, end: Int) -> Int {
    let mut index = start
    let mut in_string = 0
    let mut escaped = 0
    while index < end {
        let ch = bytes.string_load8(text, index)
        if in_string != 0 {
            if escaped != 0 {
                escaped = 0
            } else {
                if ch == 92 {
                    escaped = 1
                }
                if ch == 34 {
                    in_string = 0
                }
            }
        } else {
            if ch == 35 {
                return index
            }
            if ch == 34 {
                in_string = 1
            }
        }
        index += 1
    }
    return end
}

fn trim_start_range(text: &Str, start: Int, end: Int) -> Int {
    let mut index = start
    while index < end && bytes.is_space(bytes.string_load8(text, index)) {
        index += 1
    }
    return index
}

fn trim_end_range(text: &Str, start: Int, end: Int) -> Int {
    let mut index = end
    while index > start && bytes.is_space(bytes.string_load8(text, index - 1)) {
        index -= 1
    }
    return index
}

fn find_byte_range(text: &Str, start: Int, end: Int, byte: Int) -> Int {
    let mut index = start
    while index < end {
        if bytes.string_load8(text, index) == byte {
            return index
        }
        index += 1
    }
    return 0 - 1
}

fn is_section_range(text: &Str, start: Int, end: Int) -> Bool {
    if end - start < 2 {
        return false
    }
    return bytes.string_load8(text, start) == 91 && bytes.string_load8(text, end - 1) == 93
}

fn is_quoted_range(text: &Str, start: Int, end: Int) -> Bool {
    if end - start < 2 {
        return false
    }
    return bytes.string_load8(text, start) == 34 && bytes.string_load8(text, end - 1) == 34
}

fn is_int_range(text: &Str, start: Int, end: Int) -> Bool {
    if end <= start {
        return false
    }
    let mut index = start
    if bytes.string_load8(text, index) == 45 {
        index += 1
        if index >= end {
            return false
        }
    }
    while index < end {
        if !bytes.is_digit(bytes.string_load8(text, index)) {
            return false
        }
        index += 1
    }
    return true
}

fn valid_value_range(text: &Str, start: Int, end: Int) -> Bool {
    if is_quoted_range(text, start, end) {
        return valid_string_range(text, start, end)
    }
    if is_int_range(text, start, end) {
        return true
    }
    if range_equals(text, start, end, "true") {
        return true
    }
    if range_equals(text, start, end, "false") {
        return true
    }
    if end - start >= 2 && bytes.string_load8(text, start) == 91 && bytes.string_load8(text, end - 1) == 93 {
        return valid_array_range(text, start, end)
    }
    return false
}

fn valid_string_range(text: &Str, start: Int, end: Int) -> Bool {
    let mut index = start + 1
    let mut escaped = false
    while index < end - 1 {
        let ch = bytes.string_load8(text, index)
        if escaped {
            escaped = false
        } else {
            if ch == 92 {
                escaped = true
            }
            if ch == 34 {
                return false
            }
        }
        index += 1
    }
    return !escaped
}

fn valid_array_range(text: &Str, start: Int, end: Int) -> Bool {
    let mut item_start = start + 1
    let mut index = start + 1
    let mut in_string = false
    let mut escaped = false
    while index <= end - 1 {
        let at_end = index == end - 1
        let ch = if at_end { 44 } else { bytes.string_load8(text, index) }
        if in_string {
            if escaped {
                escaped = false
            } else {
                if ch == 92 {
                    escaped = true
                }
                if ch == 34 {
                    in_string = false
                }
            }
        } else {
            if ch == 34 {
                in_string = true
            }
            if ch == 44 {
                let item_trim_start = trim_start_range(text, item_start, index)
                let item_trim_end = trim_end_range(text, item_trim_start, index)
                if item_trim_end > item_trim_start {
                    if !valid_value_range(text, item_trim_start, item_trim_end) {
                        return false
                    }
                }
                item_start = index + 1
            }
        }
        index += 1
    }
    return true
}

fn range_equals(text: &Str, start: Int, end: Int, pattern: &Str) -> Bool {
    if end - start != String.len(pattern) {
        return false
    }
    let mut index = 0
    while index < String.len(pattern) {
        if bytes.string_load8(text, start + index) != bytes.string_load8(pattern, index) {
            return false
        }
        index += 1
    }
    return true
}

fn key_matches_range(
    text: &Str,
    has_section: Bool,
    section_start: Int,
    section_end: Int,
    key_start: Int,
    key_end: Int,
    wanted: &Str,
) -> Bool {
    if !has_section {
        return range_equals(text, key_start, key_end, wanted)
    }
    let section_len = section_end - section_start
    let key_len = key_end - key_start
    if String.len(wanted) != section_len + 1 + key_len {
        return false
    }
    let mut index = 0
    while index < section_len {
        if bytes.string_load8(wanted, index) != bytes.string_load8(text, section_start + index) {
            return false
        }
        index += 1
    }
    if bytes.string_load8(wanted, section_len) != 46 {
        return false
    }
    index = 0
    while index < key_len {
        if bytes.string_load8(wanted, section_len + 1 + index) != bytes.string_load8(text, key_start + index) {
            return false
        }
        index += 1
    }
    return true
}

fn section_key_matches(
    text: &Str,
    section_start: Int,
    section_end: Int,
    key_start: Int,
    key_end: Int,
    wanted: &Str,
) -> Bool {
    let section_len = section_end - section_start
    let key_len = key_end - key_start
    if String.len(wanted) != section_len + 1 + key_len {
        return false
    }
    let mut index = 0
    while index < section_len {
        if bytes.string_load8(wanted, index) != bytes.string_load8(text, section_start + index) {
            return false
        }
        index += 1
    }
    if bytes.string_load8(wanted, section_len) != 46 {
        return false
    }
    index = 0
    while index < key_len {
        if bytes.string_load8(wanted, section_len + 1 + index) != bytes.string_load8(text, key_start + index) {
            return false
        }
        index += 1
    }
    return true
}

fn range_equals_key(text: &Str, start: Int, end: Int, key: &Str) -> Bool {
    if end - start != String.len(key) {
        return false
    }
    let mut index = 0
    while index < String.len(key) {
        if bytes.string_load8(text, start + index) != bytes.string_load8(key, index) {
            return false
        }
        index += 1
    }
    return true
}

fn key_suffix_matches(text: &Str, start: Int, end: Int, wanted: &Str) -> Bool {
    let mut wanted_start = String.len(wanted)
    while wanted_start > 0 {
        if bytes.string_load8(wanted, wanted_start - 1) == 46 {
            break
        }
        wanted_start -= 1
    }
    let suffix_start = if wanted_start > 0 { wanted_start } else { 0 }
    if end - start != String.len(wanted) - suffix_start {
        return false
    }
    let mut index = 0
    while index < end - start {
        if bytes.string_load8(text, start + index) != bytes.string_load8(wanted, suffix_start + index) {
            return false
        }
        index += 1
    }
    return true
}

fn parse_string_range(text: &Str, start: Int, end: Int) -> String {
    let mut out = String.from("")
    let mut index = start + 1
    let mut escaped = 0
    while index < end - 1 {
        let ch = bytes.string_load8(text, index)
        if escaped != 0 {
            if ch == 110 {
                out = String.concat(out, "\n")
            } else if ch == 116 {
                out = String.concat(out, "\t")
            } else {
                out = String.concat(out, String.from_byte(ch))
            }
            escaped = 0
        } else {
            if ch == 92 {
                escaped = 1
            } else {
                out = String.concat(out, String.from_byte(ch))
            }
        }
        index += 1
    }
    return out
}

fn find_value_exists(text: &Str, wanted: &Str) -> Bool {
    let mut section = String.from("")
    let mut index = 0
    let mut line_number = 1
    let len = String.len(text)
    while index < len {
        let end = logical_line_end(text, index)
        if end == index {
            line_number += 1
            index = logical_line_next(text, end)
            continue
        }
        let raw = line_without_comment(text, index, end)
        let cleaned = trim(raw)
        if String.len(cleaned) > 0 {
            if is_section(cleaned) {
                section = section_name(cleaned)
            } else {
                let eq = find_byte(cleaned, 61)
                if eq > 0 {
                    let local_key = trim(substring(cleaned, 0, eq))
                    if key_matches(section, local_key, wanted) {
                        return true
                    }
                }
            }
        }
        line_number += count_newlines(text, index, end) + 1
        index = logical_line_next(text, end)
    }
    return false
}

fn array_item(text: &Str, key: &Str, wanted_index: Int) -> String {
    if wanted_index < 0 {
        return String.from("")
    }
    let value = find_value(text, key)
    if !is_array(value) {
        return String.from("")
    }
    let inner = substring(value, 1, String.len(value) - 1)
    let mut start = 0
    let mut index = 0
    let mut current = 0
    let mut in_string = false
    let mut escaped = false
    while index <= String.len(inner) {
        let at_end = index == String.len(inner)
        let ch = if at_end { 44 } else { bytes.string_load8(inner, index) }
        if in_string {
            if escaped {
                escaped = false
            } else {
                if ch == 92 {
                    escaped = true
                }
                if ch == 34 {
                    in_string = false
                }
            }
        } else {
            if ch == 34 {
                in_string = true
            }
            if ch == 44 {
                if current == wanted_index {
                    return trim(substring(inner, start, index))
                }
                current += 1
                start = index + 1
            }
        }
        index += 1
    }
    return String.from("")
}

fn array_item_start(text: &Str, key: &Str, wanted_index: Int) -> Int {
    let value_start = find_value_start(text, key)
    if value_start < 0 {
        return 0 - 1
    }
    let value_end = find_value_end(text, key)
    return array_item_start_in_value(text, value_start, value_end, wanted_index)
}

fn array_item_end(text: &Str, key: &Str, wanted_index: Int) -> Int {
    let value_start = find_value_start(text, key)
    if value_start < 0 {
        return 0 - 1
    }
    let value_end = find_value_end(text, key)
    return array_item_end_in_value(text, value_start, value_end, wanted_index)
}

fn array_len_range(text: &Str, start: Int, end: Int) -> Int {
    if end - start < 2 {
        return 0
    }
    if bytes.string_load8(text, start) != 91 {
        return 0
    }
    if bytes.string_load8(text, end - 1) != 93 {
        return 0
    }
    let inner_start = trim_start_range(text, start + 1, end - 1)
    let inner_end = trim_end_range(text, inner_start, end - 1)
    if inner_end <= inner_start {
        return 0
    }
    let mut count = 1
    let mut index = inner_start
    let mut last_comma = 0
    while index < end - 1 {
        if bytes.string_load8(text, index) == 44 {
            count += 1
            last_comma = index
        }
        index += 1
    }
    if last_comma > 0 {
        let tail_start = trim_start_range(text, last_comma + 1, end - 1)
        let tail_end = trim_end_range(text, tail_start, end - 1)
        if tail_end <= tail_start {
            count -= 1
        }
    }
    return count
}

fn array_value_end(text: &Str, start: Int, fallback_end: Int) -> Int {
    if start < 0 {
        return fallback_end
    }
    if start >= String.len(text) {
        return fallback_end
    }
    if bytes.string_load8(text, start) != 91 {
        return fallback_end
    }
    let mut index = start + 1
    let len = String.len(text)
    while index < len {
        if bytes.string_load8(text, index) == 93 {
            return index + 1
        }
        index += 1
    }
    return fallback_end
}

fn string_value_end(text: &Str, start: Int, fallback_end: Int) -> Int {
    let mut index = start + 1
    let len = String.len(text)
    let mut escaped = 0
    while index < len {
        let ch = bytes.string_load8(text, index)
        if escaped != 0 {
            escaped = 0
        } else {
            if ch == 92 {
                escaped = 1
            }
            if ch == 34 {
                return index + 1
            }
            if is_newline(ch) {
                return fallback_end
            }
        }
        index += 1
    }
    return fallback_end
}

fn array_item_start_in_value(text: &Str, value_start: Int, value_end: Int, wanted_index: Int) -> Int {
    if wanted_index < 0 {
        return 0 - 1
    }
    if value_end - value_start < 2 {
        return 0 - 1
    }
    if bytes.string_load8(text, value_start) != 91 {
        return 0 - 1
    }
    let mut item_start = value_start + 1
    let mut index = value_start + 1
    let mut current = 0
    while index <= value_end - 1 {
        let at_end = index == value_end - 1
        let ch = if at_end { 44 } else { bytes.string_load8(text, index) }
        if ch == 44 {
            if current == wanted_index {
                return trim_start_range(text, item_start, index)
            }
            current += 1
            item_start = index + 1
        }
        index += 1
    }
    return 0 - 1
}

fn array_item_end_in_value(text: &Str, value_start: Int, value_end: Int, wanted_index: Int) -> Int {
    if wanted_index < 0 {
        return 0 - 1
    }
    if value_end - value_start < 2 {
        return 0 - 1
    }
    if bytes.string_load8(text, value_start) != 91 {
        return 0 - 1
    }
    let mut item_start = value_start + 1
    let mut index = value_start + 1
    let mut current = 0
    while index <= value_end - 1 {
        let at_end = index == value_end - 1
        let ch = if at_end { 44 } else { bytes.string_load8(text, index) }
        if ch == 44 {
            if current == wanted_index {
                let start = trim_start_range(text, item_start, index)
                return trim_end_range(text, start, index)
            }
            current += 1
            item_start = index + 1
        }
        index += 1
    }
    return 0 - 1
}

fn valid_value(value: &Str) -> Bool {
    if is_quoted(value) {
        return valid_string(value)
    }
    if is_int_value(value) {
        return true
    }
    if value == "true" {
        return true
    }
    if value == "false" {
        return true
    }
    if is_array(value) {
        return valid_array(value)
    }
    return false
}

fn valid_array(value: &Str) -> Bool {
    let inner = trim(substring(value, 1, String.len(value) - 1))
    if String.len(inner) == 0 {
        return true
    }
    let mut index = 0
    while index < array_len_raw(inner) {
        if !valid_value(array_item_raw(inner, index)) {
            return false
        }
        index += 1
    }
    return true
}

fn array_len_raw(inner: &Str) -> Int {
    if String.len(trim(inner)) == 0 {
        return 0
    }
    let mut count = 1
    let mut index = 0
    let mut in_string = false
    let mut escaped = false
    while index < String.len(inner) {
        let ch = bytes.string_load8(inner, index)
        if in_string {
            if escaped {
                escaped = false
            } else {
                if ch == 92 {
                    escaped = true
                }
                if ch == 34 {
                    in_string = false
                }
            }
        } else {
            if ch == 34 {
                in_string = true
            }
            if ch == 44 {
                count += 1
            }
        }
        index += 1
    }
    return count
}

fn array_item_raw(inner: &Str, wanted_index: Int) -> String {
    let mut start = 0
    let mut index = 0
    let mut current = 0
    let mut in_string = false
    let mut escaped = false
    while index <= String.len(inner) {
        let at_end = index == String.len(inner)
        let ch = if at_end { 44 } else { bytes.string_load8(inner, index) }
        if in_string {
            if escaped {
                escaped = false
            } else {
                if ch == 92 {
                    escaped = true
                }
                if ch == 34 {
                    in_string = false
                }
            }
        } else {
            if ch == 34 {
                in_string = true
            }
            if ch == 44 {
                if current == wanted_index {
                    return trim(substring(inner, start, index))
                }
                current += 1
                start = index + 1
            }
        }
        index += 1
    }
    return String.from("")
}

fn logical_line_end(text: &Str, start: Int) -> Int {
    let mut index = start
    let len = String.len(text)
    while index < len {
        if is_newline(bytes.string_load8(text, index)) {
            return index
        }
        index += 1
    }
    return index
}

fn logical_line_next(text: &Str, end: Int) -> Int {
    if end < String.len(text) {
        if is_newline(bytes.string_load8(text, end)) {
            return end + 1
        }
    }
    return end
}

fn line_without_comment(text: &Str, start: Int, end: Int) -> String {
    let mut index = start
    let mut in_string = 0
    let mut escaped = 0
    while index < end {
        let ch = bytes.string_load8(text, index)
        if in_string != 0 {
            if escaped != 0 {
                escaped = 0
            } else {
                if ch == 92 {
                    escaped = 1
                }
                if ch == 34 {
                    in_string = 0
                }
            }
        } else {
            if ch == 35 {
                return substring(text, start, index)
            }
            if ch == 34 {
                in_string = 1
            }
        }
        index += 1
    }
    return substring(text, start, end)
}

fn count_newlines(text: &Str, start: Int, end: Int) -> Int {
    let mut count = 0
    let mut index = start
    while index < end {
        if is_newline(bytes.string_load8(text, index)) {
            count += 1
        }
        index += 1
    }
    return count
}

fn strip_comment(text: &Str) -> String {
    let mut out = buffer.string_builder_new(String.len(text))
    let mut index = 0
    let mut in_string = false
    let mut escaped = false
    while index < String.len(text) {
        let ch = bytes.string_load8(text, index)
        if in_string {
            buffer.string_builder_append_byte(out, ch)
            if escaped {
                escaped = false
            } else {
                if ch == 92 {
                    escaped = true
                }
                if ch == 34 {
                    in_string = false
                }
            }
        } else {
            if ch == 35 {
                return buffer.string_builder_finish(out)
            }
            buffer.string_builder_append_byte(out, ch)
            if ch == 34 {
                in_string = true
            }
        }
        index += 1
    }
    return buffer.string_builder_finish(out)
}

fn is_newline(byte: Int) -> Bool {
    if byte == 10 {
        return true
    }
    return byte == 1
}

fn valid_section(text: &Str) -> Bool {
    if !is_section(text) {
        return false
    }
    return String.len(section_name(text)) > 0
}

fn is_section(text: &Str) -> Bool {
    let len = String.len(text)
    if len < 2 {
        return false
    }
    return bytes.string_load8(text, 0) == 91 && bytes.string_load8(text, len - 1) == 93
}

fn section_name(text: &Str) -> String {
    return trim(substring(text, 1, String.len(text) - 1))
}

fn key_matches(section: &Str, key: &Str, wanted: &Str) -> Bool {
    if String.len(section) == 0 {
        return key == wanted
    }
    let expected_len = String.len(section) + 1 + String.len(key)
    if String.len(wanted) != expected_len {
        return false
    }
    let mut index = 0
    while index < String.len(section) {
        if bytes.string_load8(wanted, index) != bytes.string_load8(section, index) {
            return false
        }
        index += 1
    }
    if bytes.string_load8(wanted, index) != 46 {
        return false
    }
    index += 1
    let mut key_index = 0
    while key_index < String.len(key) {
        if bytes.string_load8(wanted, index + key_index) != bytes.string_load8(key, key_index) {
            return false
        }
        key_index += 1
    }
    return true
}

fn is_array(value: &Str) -> Bool {
    let len = String.len(value)
    if len < 2 {
        return false
    }
    return bytes.string_load8(value, 0) == 91 && bytes.string_load8(value, len - 1) == 93
}

fn is_quoted(value: &Str) -> Bool {
    let len = String.len(value)
    if len < 2 {
        return false
    }
    return bytes.string_load8(value, 0) == 34 && bytes.string_load8(value, len - 1) == 34
}

fn valid_string(value: &Str) -> Bool {
    if !is_quoted(value) {
        return false
    }
    let mut index = 1
    let mut escaped = false
    while index < String.len(value) - 1 {
        let ch = bytes.string_load8(value, index)
        if escaped {
            escaped = false
        } else {
            if ch == 92 {
                escaped = true
            }
            if ch == 34 {
                return false
            }
        }
        index += 1
    }
    return !escaped
}

fn parse_string(value: &Str) -> String {
    let mut out = String.from("")
    let mut index = 1
    while index < String.len(value) - 1 {
        let ch = bytes.string_load8(value, index)
        if ch == 92 {
            let next = bytes.string_load8(value, index + 1)
            if next == 110 {
                out = String.concat(out, "\n")
            } else if next == 116 {
                out = String.concat(out, "\t")
            } else {
                out = String.concat(out, String.from_byte(next))
            }
            index += 2
        } else {
            out = String.concat(out, String.from_byte(ch))
            index += 1
        }
    }
    return out
}

fn is_int_value(value: &Str) -> Bool {
    let len = String.len(value)
    if len == 0 {
        return false
    }
    let mut index = 0
    if bytes.string_load8(value, 0) == 45 {
        if len == 1 {
            return false
        }
        index = 1
    }
    while index < len {
        if !bytes.is_digit(bytes.string_load8(value, index)) {
            return false
        }
        index += 1
    }
    return true
}

fn find_byte(text: &Str, byte: Int) -> Int {
    let mut index = 0
    while index < String.len(text) {
        if bytes.string_load8(text, index) == byte {
            return index
        }
        index += 1
    }
    return 0 - 1
}

fn trim(text: &Str) -> String {
    let mut start = 0
    let mut end = String.len(text)
    while start < end && bytes.is_space(bytes.string_load8(text, start)) {
        start += 1
    }
    while end > start && bytes.is_space(bytes.string_load8(text, end - 1)) {
        end -= 1
    }
    return substring(text, start, end)
}

fn substring(text: &Str, start: Int, end: Int) -> String {
    let mut out = buffer.string_builder_new(end - start + 1)
    let mut index = start
    while index < end {
        buffer.string_builder_append_byte(out, bytes.string_load8(text, index))
        index += 1
    }
    return buffer.string_builder_finish(out)
}
