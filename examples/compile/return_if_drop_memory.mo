import * as core from "core/unsafe"
import * as String from "std/string"

fn choose(flag: Bool) -> Int {
    return if flag {
        let inner = String.from("Ada")
        String.len(inner)
    } else {
        let fallback = String.from("Grace")
        String.len(fallback)
    }
}

fn main() -> Int {
    let alloc0 = core.mem_alloc_count()
    let free0 = core.mem_free_count()
    let live0 = core.mem_live_bytes()
    let len = choose(false)
    let alloc1 = core.mem_alloc_count()
    let free1 = core.mem_free_count()
    let live1 = core.mem_live_bytes()

    if len == 5 {
        if alloc1 >= alloc0 + 1 {
            if free1 >= free0 + 1 {
                if live1 == live0 {
                    return 42
                }
            }
        }
    }
    return 1
}
