import * as channel from "std/channel"
import * as core from "core/unsafe"

fn exercise_channel() -> Int {
    let ch = channel.int()
    let closed = channel.close(ch)
    let destroyed = channel.destroy(ch)
    if closed != 0 {
        return 11
    }
    if destroyed != 0 {
        return 12
    }
    return 0
}

fn main() -> Int {
    let alloc0 = core.mem_alloc_count()
    let free0 = core.mem_free_count()
    let live0 = core.mem_live_bytes()

    let result = exercise_channel()

    let alloc1 = core.mem_alloc_count()
    let free1 = core.mem_free_count()
    let live1 = core.mem_live_bytes()

    if result != 0 {
        return result
    }
    if alloc1 < alloc0 + 4 {
        return 13
    }
    if free1 < free0 + 4 {
        return 14
    }
    if live1 > live0 + 16 {
        return 15
    }
    return 42
}
