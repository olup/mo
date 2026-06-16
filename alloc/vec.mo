import * as core from "core/unsafe"

pub fn allocate_slots(capacity: Int) -> Int {
    return core.alloc(capacity * 8)
}

pub fn free_slots(data: Int) {
    core.free(data)
}

pub fn load_int(data: Int, index: Int) -> Int {
    return core.load64(data, index * 8)
}

pub fn store_int(data: Int, index: Int, value: Int) {
    core.store64(data, index * 8, value)
}

pub fn store_string(data: Int, index: Int, value: String) {
    core.store64(data, index * 8, core.string_ptr(value))
}

pub fn load_string(data: Int, index: Int) -> String {
    return core.string_from_ptr(core.load64(data, index * 8))
}

pub fn free_string_at(data: Int, index: Int) {
    core.free(core.load64(data, index * 8))
}

pub fn store_handler(data: Int, index: Int, value: fn(Int, &Str) -> Int) {
    core.store64(data, index * 8, core.function_ptr_handler(value))
}

pub fn load_handler(data: Int, index: Int) -> fn(Int, &Str) -> Int {
    return core.function_from_ptr_handler(core.load64(data, index * 8))
}

pub fn store_request_handler(data: Int, index: Int, value: fn(Int, &http__Request, &Str) -> Int) {
    core.store64(data, index * 8, core.function_ptr_request_handler(value))
}

pub fn load_request_handler(data: Int, index: Int) -> fn(Int, &http__Request, &Str) -> Int {
    return core.function_from_ptr_request_handler(core.load64(data, index * 8))
}
