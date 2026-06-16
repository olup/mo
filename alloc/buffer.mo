import * as alloc_string from "alloc/string"
import * as core from "core/unsafe"

pub fn allocate(capacity: Int) -> String {
    let data = core.alloc_string(capacity + 1)
    core.string_store8(data, 0, 0)
    return data
}

pub fn load(data: &Str, offset: Int) -> Int {
    return core.string_load8(data, offset)
}

pub fn store(data: &String, offset: Int, byte: Int) {
    core.string_store8(data, offset, byte)
}

pub fn free(data: &String) {
    alloc_string.free(data)
}
