import * as box from "std/box"
import * as String from "std/string"
import * as core from "core/unsafe"

fn exercise() -> Int {
    let text = String.concat("box", " take")
    let value: box.Box<String> = box.new(text)
    let out: String = box.take(value)
    let len = String.len(out)
    return len
}

fn main() -> Int {
    let alloc0 = core.mem_alloc_count()
    let free0 = core.mem_free_count()
    let live0 = core.mem_live_bytes()
    let len = exercise()
    let alloc1 = core.mem_alloc_count()
    let free1 = core.mem_free_count()
    let live1 = core.mem_live_bytes()
    if len != 8 {
        return 1
    }
    if alloc1 < alloc0 + 3 {
        return 2
    }
    if free1 < free0 + 3 {
        return 3
    }
    if live1 != live0 {
        return 4
    }
    return 42
}
