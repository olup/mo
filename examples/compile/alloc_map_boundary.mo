import * as alloc_map from "alloc/map"
import * as String from "std/string"

fn main() -> Int {
    let keys = alloc_map.new_string_keys()
    let values = alloc_map.new_string_values()
    alloc_map.put_string_string(keys, values, String.from("route"), String.from("health"))
    alloc_map.put_string_string(keys, values, String.from("route"), String.from("pokemon"))
    let found = alloc_map.get_string_string(keys, values, "route")
    let result = String.len(found) + alloc_map.length_string_string(keys)
    if result == 9 {
        return 42
    }
    return 1
}
