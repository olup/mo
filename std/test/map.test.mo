import * as map from "std/map"
import * as String from "std/string"

test "std map stores and reads owned string values" {
    let values: map.Map<String, String> = map.new<String, String>()
    map.put<String, String>(values, String.from("route"), String.from("health"))
    map.put<String, String>(values, String.from("route"), String.from("pokemon"))
    assert(map.length<String, String>(values) == 2)
    let found = map.get<String, String>(values, "route")
    let missing = map.get<String, String>(values, "missing")
    assert(found == "pokemon")
    assert(String.len(missing) == 0)
    map.destroy<String, String>(values)
}
