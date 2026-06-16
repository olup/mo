import * as core from "core/unsafe"
import * as option from "std/option"
import * as result from "std/result"
import * as String from "std/string"

fn keep_owned(value: String) -> option.Option<String> {
    return Some(value)
}

fn fallback_owned() -> option.Option<String> {
    return Some(String.from("fallback"))
}

fn ok_owned(value: String) -> result.Result<String, String> {
    return Ok(value)
}

fn error_length_string(value: String) -> String {
    return String.from_int(String.len(value))
}

fn option_and_then_len() -> Int {
    let value: option.Option<String> = option.and_then(Some(String.from("owned")), keep_owned)
    return match value {
        Some(item) => String.len(item)
        None => 0
    }
}

fn option_or_else_len() -> Int {
    let value: option.Option<String> = option.or_else(None, fallback_owned)
    return match value {
        Some(item) => String.len(item)
        None => 0
    }
}

fn result_and_then_len() -> Int {
    let value: result.Result<String, String> = result.and_then(Ok(String.from("owned")), ok_owned)
    return match value {
        Ok(item) => String.len(item)
        Err(error) => String.len(error)
    }
}

fn result_map_err_len() -> Int {
    let value: result.Result<String, String> = result.map_err(Err(String.from("error")), error_length_string)
    return match value {
        Ok(item) => String.len(item)
        Err(error) => String.len(error)
    }
}

fn run_all() -> Int {
    let a = option_and_then_len()
    let b = option_or_else_len()
    let c = result_and_then_len()
    let d = result_map_err_len()
    return a + b + c + d
}

fn main() -> Int {
    let before = core.mem_live_bytes()
    let total = run_all()
    let after = core.mem_live_bytes()
    if total != 19 {
        return 1
    }
    if after != before {
        return 2
    }
    return 42
}
