import * as core from "core/unsafe"
import * as map from "std/map"
import * as String from "std/string"

fn exercise() -> Int {
    let values: map.Map<String, String> = map.new<String, String>()
    let first_key = String.from("starter")
    let first_value = String.from("bulbasaur")
    let second_key = String.from("starter")
    let second_value = String.from("pikachu")
    map.put<String, String>(values, first_key, first_value)
    map.put<String, String>(values, second_key, second_value)
    return String.len(map.get<String, String>(values, "starter")) + map.length<String, String>(values)
}

fn main() -> Int {
    let alloc0 = core.mem_alloc_count()
    let free0 = core.mem_free_count()
    let result = exercise()
    let alloc1 = core.mem_alloc_count()
    let free1 = core.mem_free_count()
    if result != 9 {
        return 1
    }
    if alloc1 < alloc0 + 8 {
        return 2
    }
    if free1 < free0 + 8 {
        return 3
    }
    return 42
}
