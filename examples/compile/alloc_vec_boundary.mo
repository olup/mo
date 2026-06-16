import * as alloc_vec from "alloc/vec"

fn main() -> Int {
    let data = alloc_vec.allocate_slots(2)
    alloc_vec.store_int(data, 0, 10)
    alloc_vec.store_int(data, 1, 32)
    let value = alloc_vec.load_int(data, 0) + alloc_vec.load_int(data, 1)
    alloc_vec.free_slots(data)
    return value
}
