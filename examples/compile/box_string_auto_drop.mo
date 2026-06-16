import * as box from "std/box"
import * as String from "std/string"
import * as core from "core/unsafe"

fn exercise() {
    let text = String.concat("box", " string")
    let value: box.Box<String> = box.new(text)
}

fn main() -> Int {
    let alloc0 = core.mem_alloc_count()
    let free0 = core.mem_free_count()
    exercise()
    let alloc1 = core.mem_alloc_count()
    let free1 = core.mem_free_count()
    if alloc1 < alloc0 + 3 {
        return 1
    }
    if free1 < free0 + 3 {
        return 2
    }
    return 42
}
