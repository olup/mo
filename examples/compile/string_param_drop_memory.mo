import * as core from "core/unsafe"
import * as String from "std/string"

fn consume(value: String) -> Int {
    return String.len(value)
}

fn main() -> Int {
    let alloc0 = core.mem_alloc_count()
    let free0 = core.mem_free_count()
    let live0 = core.mem_live_bytes()
    let len = consume(String.from("Pikachu"))
    let alloc1 = core.mem_alloc_count()
    let free1 = core.mem_free_count()
    let live1 = core.mem_live_bytes()

    if len != 7 {
        return 1
    }
    if alloc1 < alloc0 + 1 {
        return 2
    }
    if free1 < free0 + 1 {
        return 3
    }
    if live1 != live0 {
        return 4
    }
    return 42
}
