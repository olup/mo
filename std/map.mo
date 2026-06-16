import * as alloc_map from "alloc/map"
import * as String from "std/string"
import * as vec from "std/vec"

pub struct Map<K, V> {
    keys: vec.Vec<String>
    values: vec.Vec<String>
}

pub fn new<K, V>() -> Map<K, V>
pub fn length<K, V>(map: &Map<K, V>) -> Int
pub fn put<K, V>(map: &mut Map<K, V>, key: K, value: V) -> Int
pub fn get<K, V>(map: &Map<K, V>, key: &Str) -> V
pub fn destroy<K, V>(map: &Map<K, V>)

pub fn new_string_string() -> Map<String, String> {
    return Map { keys: alloc_map.new_string_keys(), values: alloc_map.new_string_values() }
}

pub fn length_string_string(map: &Map<String, String>) -> Int {
    return alloc_map.length_string_string(map.keys)
}

pub fn put_string_string(map: &mut Map<String, String>, key: String, value: String) -> Int {
    return alloc_map.put_string_string(map.keys, map.values, key, value)
}

pub fn get_string_string(map: &Map<String, String>, key: &Str) -> String {
    return alloc_map.get_string_string(map.keys, map.values, key)
}

pub fn destroy_string_string(map: &Map<String, String>) {
    alloc_map.destroy_string_string(map.keys, map.values)
}
