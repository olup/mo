import * as alloc_box from "alloc/box"

pub struct Box<T> {
    ptr: Int
}

pub fn new<T>(value: T) -> Box<T>
pub fn take<T>(value: Box<T>) -> T
pub fn destroy<T>(value: &Box<T>)

pub fn new_int(value: Int) -> Box<Int> {
    let ptr = alloc_box.allocate_cell()
    alloc_box.store_int(ptr, value)
    return Box { ptr: ptr }
}

pub fn new_string(value: String) -> Box<String> {
    let ptr = alloc_box.allocate_cell()
    alloc_box.store_string(ptr, value)
    return Box { ptr: ptr }
}

pub fn get_int(value: &Box<Int>) -> Int {
    return alloc_box.load_int(value.ptr)
}

pub fn take_int(value: &Box<Int>) -> Int {
    return alloc_box.load_int(value.ptr)
}

pub fn take_string(value: &Box<String>) -> String {
    return alloc_box.load_string(value.ptr)
}

pub fn destroy_int(value: &Box<Int>) {
    alloc_box.free_cell(value.ptr)
}
