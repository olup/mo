import * as alloc_vec from "alloc/vec"
import * as String from "std/string"

pub struct Vec<T> {
    pub data: Int
    pub length_value: Int
    pub capacity_value: Int
}

pub fn new<T>() -> Vec<T>
pub fn data<T>(values: &Vec<T>) -> Int
pub fn length<T>(values: &Vec<T>) -> Int
pub fn capacity<T>(values: &Vec<T>) -> Int
pub fn push<T>(vec: &mut Vec<T>, value: T) -> Int
pub fn get<T>(values: &Vec<T>, index: Int) -> T
pub fn destroy<T>(vec: &Vec<T>)

pub fn new_int() -> Vec<Int> {
    return Vec { data: 0, length_value: 0, capacity_value: 0 }
}

pub fn new_string() -> Vec<String> {
    return Vec { data: 0, length_value: 0, capacity_value: 0 }
}

pub fn new_handler() -> Vec<fn(Int, &Str) -> Int> {
    return Vec { data: 0, length_value: 0, capacity_value: 0 }
}

pub fn new_request_handler() -> Vec<fn(Int, &http__Request, &Str) -> Int> {
    return Vec { data: 0, length_value: 0, capacity_value: 0 }
}


pub fn length_int(values: &Vec<Int>) -> Int {
    return values.length_value
}

pub fn data_int(values: &Vec<Int>) -> Int {
    return values.data
}

pub fn length_string(values: &Vec<String>) -> Int {
    return values.length_value
}

pub fn data_string(values: &Vec<String>) -> Int {
    return values.data
}

pub fn length_handler(values: &Vec<fn(Int, &Str) -> Int>) -> Int {
    return values.length_value
}

pub fn data_handler(values: &Vec<fn(Int, &Str) -> Int>) -> Int {
    return values.data
}

pub fn length_request_handler(values: &Vec<fn(Int, &http__Request, &Str) -> Int>) -> Int {
    return values.length_value
}

pub fn data_request_handler(values: &Vec<fn(Int, &http__Request, &Str) -> Int>) -> Int {
    return values.data
}


pub fn capacity_int(values: &Vec<Int>) -> Int {
    return values.capacity_value
}

pub fn capacity_string(values: &Vec<String>) -> Int {
    return values.capacity_value
}

pub fn capacity_handler(values: &Vec<fn(Int, &Str) -> Int>) -> Int {
    return values.capacity_value
}

pub fn capacity_request_handler(values: &Vec<fn(Int, &http__Request, &Str) -> Int>) -> Int {
    return values.capacity_value
}


fn grow_int(values: &mut Vec<Int>, needed: Int) -> Int {
    if needed <= values.capacity_value {
        return values.capacity_value
    }
    let mut next_capacity = values.capacity_value + values.capacity_value + 1
    while next_capacity < needed {
        next_capacity *= 2
    }
    let next = alloc_vec.allocate_slots(next_capacity)
    let mut index = 0
    while index < values.length_value {
        alloc_vec.store_int(next, index, alloc_vec.load_int(values.data, index))
        index += 1
    }
    if values.data != 0 {
        alloc_vec.free_slots(values.data)
    }
    values.data = next
    values.capacity_value = next_capacity
    return next_capacity
}

fn grow_string(values: &mut Vec<String>, needed: Int) -> Int {
    if needed <= values.capacity_value {
        return values.capacity_value
    }
    let mut next_capacity = values.capacity_value + values.capacity_value + 1
    while next_capacity < needed {
        next_capacity *= 2
    }
    let next = alloc_vec.allocate_slots(next_capacity)
    let mut index = 0
    while index < values.length_value {
        alloc_vec.store_int(next, index, alloc_vec.load_int(values.data, index))
        index += 1
    }
    if values.data != 0 {
        alloc_vec.free_slots(values.data)
    }
    values.data = next
    values.capacity_value = next_capacity
    return next_capacity
}

fn grow_handler(values: &mut Vec<fn(Int, &Str) -> Int>, needed: Int) -> Int {
    if needed <= values.capacity_value {
        return values.capacity_value
    }
    let mut next_capacity = values.capacity_value + values.capacity_value + 1
    while next_capacity < needed {
        next_capacity *= 2
    }
    let next = alloc_vec.allocate_slots(next_capacity)
    let mut index = 0
    while index < values.length_value {
        alloc_vec.store_int(next, index, alloc_vec.load_int(values.data, index))
        index += 1
    }
    if values.data != 0 {
        alloc_vec.free_slots(values.data)
    }
    values.data = next
    values.capacity_value = next_capacity
    return next_capacity
}

fn grow_request_handler(values: &mut Vec<fn(Int, &http__Request, &Str) -> Int>, needed: Int) -> Int {
    if needed <= values.capacity_value {
        return values.capacity_value
    }
    let mut next_capacity = values.capacity_value + values.capacity_value + 1
    while next_capacity < needed {
        next_capacity *= 2
    }
    let next = alloc_vec.allocate_slots(next_capacity)
    let mut index = 0
    while index < values.length_value {
        alloc_vec.store_int(next, index, alloc_vec.load_int(values.data, index))
        index += 1
    }
    if values.data != 0 {
        alloc_vec.free_slots(values.data)
    }
    values.data = next
    values.capacity_value = next_capacity
    return next_capacity
}


pub fn push_int(values: &mut Vec<Int>, value: Int) -> Int {
    if values.length_value >= values.capacity_value {
        grow_int(values, values.length_value + 1)
    }
    alloc_vec.store_int(values.data, values.length_value, value)
    values.length_value += 1
    return values.length_value
}

pub fn get_int(values: &Vec<Int>, index: Int) -> Int {
    if index < 0 {
        return 0
    }
    if index >= values.length_value {
        return 0
    }
    return alloc_vec.load_int(values.data, index)
}

pub fn push_string(values: &mut Vec<String>, value: String) -> Int {
    if values.length_value >= values.capacity_value {
        grow_string(values, values.length_value + 1)
    }
    alloc_vec.store_string(values.data, values.length_value, value)
    values.length_value += 1
    return values.length_value
}

pub fn get_string(values: &Vec<String>, index: Int) -> String {
    if index < 0 {
        return String.from("")
    }
    if index >= values.length_value {
        return String.from("")
    }
    return alloc_vec.load_string(values.data, index)
}

pub fn push_handler(values: &mut Vec<fn(Int, &Str) -> Int>, value: fn(Int, &Str) -> Int) -> Int {
    if values.length_value >= values.capacity_value {
        grow_handler(values, values.length_value + 1)
    }
    alloc_vec.store_handler(values.data, values.length_value, value)
    values.length_value += 1
    return values.length_value
}

pub fn get_handler(values: &Vec<fn(Int, &Str) -> Int>, index: Int) -> fn(Int, &Str) -> Int {
    if index < 0 {
        return default_handler_value
    }
    if index >= values.length_value {
        return default_handler_value
    }
    return alloc_vec.load_handler(values.data, index)
}

fn default_handler_value(client: Int, context: &Str) -> Int {
    return 0
}

pub fn push_request_handler(values: &mut Vec<fn(Int, &http__Request, &Str) -> Int>, value: fn(Int, &http__Request, &Str) -> Int) -> Int {
    if values.length_value >= values.capacity_value {
        grow_request_handler(values, values.length_value + 1)
    }
    alloc_vec.store_request_handler(values.data, values.length_value, value)
    values.length_value += 1
    return values.length_value
}

pub fn get_request_handler(values: &Vec<fn(Int, &http__Request, &Str) -> Int>, index: Int) -> fn(Int, &http__Request, &Str) -> Int {
    if index < 0 {
        return default_request_handler_value
    }
    if index >= values.length_value {
        return default_request_handler_value
    }
    return alloc_vec.load_request_handler(values.data, index)
}

fn default_request_handler_value(client: Int, request: &http__Request, context: &Str) -> Int {
    return 0
}


pub fn destroy_int(values: &Vec<Int>) {
    if values.data != 0 {
        alloc_vec.free_slots(values.data)
    }
}

pub fn destroy_string(values: &Vec<String>) {
    let mut index = 0
    while index < values.length_value {
        alloc_vec.free_string_at(values.data, index)
        index += 1
    }
    if values.data != 0 {
        alloc_vec.free_slots(values.data)
    }
}

pub fn destroy_handler(values: &Vec<fn(Int, &Str) -> Int>) {
    if values.data != 0 {
        alloc_vec.free_slots(values.data)
    }
}

pub fn destroy_request_handler(values: &Vec<fn(Int, &http__Request, &Str) -> Int>) {
    if values.data != 0 {
        alloc_vec.free_slots(values.data)
    }
}
