import * as buffer from "std/buffer"
import * as bytes from "std/bytes"
import * as core from "core/unsafe"
import * as int from "std/int"
import * as option from "std/option"
import { Option } from "std/option"
import * as path from "std/path"
import * as result from "std/result"
import { Result } from "std/result"
import * as String from "std/string"

fn unwrap_option_or(value: Option<Int>, fallback: Int) -> Int {
    return match value {
        Some(item) => item
        None => fallback
    }
}

fn unwrap_result_or(value: Result<Int, Int>) -> Int {
    return match value {
        Ok(item) => item
        Err(error) => error
    }
}

fn unwrap_result_error_or(value: Result<Int, Int>, fallback: Int) -> Int {
    return match value {
        Ok(item) => fallback
        Err(error) => error
    }
}

fn add_one(value: Int) -> Int {
    return value + 1
}

fn string_length(value: String) -> Int {
    return String.len(value)
}

fn keep_positive(value: Int) -> option.Option<Int> {
    if value > 0 {
        return Some(value + 1)
    }
    return None
}

fn string_length_option(value: String) -> option.Option<Int> {
    return Some(String.len(value))
}

fn fallback_positive() -> option.Option<Int> {
    return Some(42)
}

fn fallback_string() -> option.Option<String> {
    return Some(String.from("fallback"))
}

fn require_positive(value: Int) -> result.Result<Int, Int> {
    if value > 0 {
        return Ok(value + 1)
    }
    return Err(5)
}

fn string_length_result(value: String) -> result.Result<Int, Int> {
    return Ok(String.len(value))
}

fn error_string_length(value: String) -> Int {
    return String.len(value)
}

fn recover_positive(error: Int) -> result.Result<Int, Int> {
    return Ok(error + 35)
}

fn recover_string_error(value: String) -> result.Result<Int, Int> {
    return Ok(String.len(value))
}

test "std string helpers" {
    let value = String.concat("hello", String.from_byte(33))
    assert(String.len(value) == 6)
    assert(bytes.string_load8(value, 0) == 104)
    assert(bytes.string_load8(value, 5) == 33)

    let number = String.from_int(42)
    assert(String.len(number) == 2)
    assert(bytes.string_load8(number, 0) == 52)
    assert(bytes.string_load8(number, 1) == 50)
}

test "string equality compares values" {
    assert("abc" == "abc")
    assert(!("abc" == "abd"))
    assert("abc" != "abd")
    assert(!("abc" != "abc"))
    assert(String.concat("a", "bc") == "abc")
    assert(String.from_int(123) == int.to_string(123))
    assert(String.from_byte(33) == "!")
}

test "bool equality compares values" {
    assert(true == true)
    assert(false == false)
    assert(!(true == false))
    assert(true != false)
    assert(!(true != true))
}

test "std byte classification and memory helpers" {
    assert(bytes.is_digit(48))
    assert(bytes.is_digit(57))
    assert(!bytes.is_digit(65))
    assert(bytes.digit_value(55) == 7)
    assert(bytes.digit_value(65) == 0 - 1)

    assert(bytes.is_alpha(65))
    assert(bytes.is_alpha(122))
    assert(!bytes.is_alpha(48))
    assert(bytes.is_space(32))
    assert(bytes.is_space(10))
    assert(!bytes.is_space(65))

    let ptr = core.alloc(4)
    bytes.store_u16_be(ptr, 0, 4660)
    assert(bytes.load_u16_be(ptr, 0) == 4660)
    bytes.store_u16_le(ptr, 0, 4660)
    assert(bytes.load_u16_le(ptr, 0) == 4660)
    bytes.store_u32_le(ptr, 0, 305419896)
    assert(bytes.load_u32_le(ptr, 0) == 305419896)
    bytes.zero(ptr, 4)
    assert(bytes.load8(ptr, 0) == 0)
    assert(bytes.load8(ptr, 3) == 0)
    core.free(ptr)
}

test "std int helpers" {
    let int_text = int.to_string(123)
    assert(String.len(int_text) == 3)
    assert(bytes.string_load8(int_text, 0) == 49)
    assert(bytes.string_load8(int_text, 1) == 50)
    assert(bytes.string_load8(int_text, 2) == 51)
    assert(int.parse_decimal_or("123", 0) == 123)
    assert(int.parse_decimal_or("-7", 0) == 0 - 7)
    assert(int.parse_decimal_or("", 9) == 9)
    assert(int.parse_decimal_or("12x", 9) == 9)

    assert(int.checked_add_or(40, 2, 0) == 42)
    assert(int.checked_sub_or(40, 2, 0) == 38)
    assert(int.checked_mul_or(6, 7, 0) == 42)
    assert(int.checked_add_or(int.max_value(), 1, 99) == 99)
    assert(int.checked_sub_or(int.min_value(), 1, 99) == 99)
    assert(int.checked_mul_or(int.max_value(), 2, 99) == 99)

    assert(int.is_i8(127))
    assert(!int.is_i8(128))
    assert(int.is_u8(255))
    assert(!int.is_u8(256))
    assert(int.to_u8_or(256, 7) == 7)
}

test "std option helpers" {
    let present: Option<Int> = Some(42)
    let missing: Option<Int> = None
    assert(unwrap_option_or(present, 0) == 42)
    assert(unwrap_option_or(missing, 7) == 7)
    assert(option.is_some(Some(1)))
    assert(!option.is_some(None))
    assert(option.is_none(None))
    assert(!option.is_none(Some(1)))
    assert(option.unwrap_or(Some(42), 0) == 42)
    assert(option.unwrap_or(None, 7) == 7)
    assert(option.unwrap_or(Some(String.from("owned")), String.from("fallback")) == "owned")
    assert(option.unwrap_or(None, String.from("fallback")) == "fallback")
    assert(option.unwrap_or(option.map(Some(41), add_one), 0) == 42)
    assert(option.unwrap_or(option.map(None, add_one), 7) == 7)
    assert(option.unwrap_or(option.map(Some(String.from("owned")), string_length), 0) == 5)
    assert(option.unwrap_or(option.and_then(Some(41), keep_positive), 0) == 42)
    assert(option.unwrap_or(option.and_then(Some(0), keep_positive), 7) == 7)
    assert(option.unwrap_or(option.and_then(None, keep_positive), 8) == 8)
    assert(option.unwrap_or(option.and_then(Some(String.from("owned")), string_length_option), 0) == 5)
    assert(option.unwrap_or(option.or_else(Some(41), fallback_positive), 0) == 41)
    assert(option.unwrap_or(option.or_else(None, fallback_positive), 0) == 42)
    assert(option.unwrap_or(option.or_else(None, fallback_string), String.from("missing")) == "fallback")
}

test "std result helpers" {
    let ok: Result<Int, Int> = Ok(42)
    let err: Result<Int, Int> = Err(7)
    assert(unwrap_result_or(ok) == 42)
    assert(unwrap_result_or(err) == 7)
    assert(result.is_ok(Ok(1)))
    assert(!result.is_ok(Err(1)))
    assert(result.is_err(Err(1)))
    assert(!result.is_err(Ok(1)))
    assert(result.unwrap_or(Ok(42), 0) == 42)
    assert(result.unwrap_or(Err(7), 9) == 9)
    assert(result.unwrap_or(Ok(String.from("owned")), String.from("fallback")) == "owned")
    assert(result.unwrap_or(Err(7), String.from("fallback")) == "fallback")
    assert(result.unwrap_or(result.map(Ok(41), add_one), 0) == 42)
    assert(result.unwrap_or(result.map(Err(7), add_one), 9) == 9)
    assert(result.unwrap_or(result.map(Ok(String.from("owned")), string_length), 0) == 5)
    assert(result.unwrap_or(result.and_then(Ok(41), require_positive), 0) == 42)
    assert(result.unwrap_or(result.and_then(Ok(0), require_positive), 9) == 9)
    assert(result.unwrap_or(result.and_then(Err(7), require_positive), 9) == 9)
    assert(result.unwrap_or(result.and_then(Ok(String.from("owned")), string_length_result), 0) == 5)
    assert(unwrap_result_error_or(result.map_err(Err(7), add_one), 0) == 8)
    assert(result.unwrap_or(result.map_err(Ok(42), add_one), 0) == 42)
    assert(unwrap_result_error_or(result.map_err(Err(String.from("owned")), error_string_length), 0) == 5)
    assert(result.unwrap_or(result.or_else(Ok(42), recover_positive), 0) == 42)
    assert(result.unwrap_or(result.or_else(Err(7), recover_positive), 0) == 42)
    assert(result.unwrap_or(result.or_else(Err(String.from("owned")), recover_string_error), 0) == 5)
}

test "std buffer helpers" {
    let buf = buffer.new(12)
    assert(buffer.append(buf, "hp") == 2)
    assert(buffer.append_byte(buf, 58) == 3)
    assert(buffer.append_int(buf, 42) == 5)
    assert(buffer.finish(buf) == "hp:42")
}


test "std path helpers" {
    assert(String.len(path.separator()) == 1)
    assert(bytes.string_load8(path.separator(), 0) == 47)

    let joined = path.join("tmp", "file")
    assert(String.len(joined) == 8)
    assert(bytes.string_load8(joined, 0) == 116)
    assert(bytes.string_load8(joined, 3) == 47)
    assert(bytes.string_load8(joined, 7) == 101)

    assert(String.len(path.join("", "file")) == 4)
    assert(String.len(path.join("tmp", "")) == 3)
}
