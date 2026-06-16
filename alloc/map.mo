import * as String from "std/string"
import * as vec from "std/vec"

pub fn new_string_keys() -> vec.Vec<String> {
    return vec.new<String>()
}

pub fn new_string_values() -> vec.Vec<String> {
    return vec.new<String>()
}

pub fn length_string_string(keys: &vec.Vec<String>) -> Int {
    return vec.length_string(keys)
}

pub fn put_string_string(keys: &mut vec.Vec<String>, values: &mut vec.Vec<String>, key: String, value: String) -> Int {
    vec.push_string(keys, key)
    return vec.push_string(values, value)
}

pub fn get_string_string(keys: &vec.Vec<String>, values: &vec.Vec<String>, key: &Str) -> String {
    let mut index = vec.length_string(keys)
    while index > 0 {
        index -= 1
        if vec.get_string(keys, index) == key {
            return String.from(vec.get_string(values, index))
        }
    }
    return String.from("")
}

pub fn destroy_string_string(keys: &vec.Vec<String>, values: &vec.Vec<String>) {
    vec.destroy_string(keys)
    vec.destroy_string(values)
}
