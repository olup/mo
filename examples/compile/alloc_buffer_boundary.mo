import * as alloc_buffer from "alloc/buffer"

fn main() -> Int {
    let data = alloc_buffer.allocate(3)
    alloc_buffer.store(data, 0, 109)
    alloc_buffer.store(data, 1, 111)
    alloc_buffer.store(data, 2, 0)
    if alloc_buffer.load(data, 0) == 109 {
        if alloc_buffer.load(data, 1) == 111 {
            return 42
        }
    }
    return 1
}
