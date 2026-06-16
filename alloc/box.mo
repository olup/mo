import * as core from "core/unsafe"

pub fn allocate_cell() -> Int {
    return core.alloc(8)
}

pub fn store_int(ptr: Int, value: Int) {
    core.store64(ptr, 0, value)
}

pub fn load_int(ptr: Int) -> Int {
    return core.load64(ptr, 0)
}

pub fn store_string(ptr: Int, value: String) {
    core.store64(ptr, 0, core.string_ptr(value))
}

pub fn load_string(ptr: Int) -> String {
    return core.string_from_ptr(core.load64(ptr, 0))
}

pub fn free_cell(ptr: Int) {
    core.free(ptr)
}
