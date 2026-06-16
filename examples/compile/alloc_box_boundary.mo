import * as alloc_box from "alloc/box"

fn main() -> Int {
    let ptr = alloc_box.allocate_cell()
    alloc_box.store_int(ptr, 42)
    let value = alloc_box.load_int(ptr)
    alloc_box.free_cell(ptr)
    return value
}
