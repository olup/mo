import * as core from "core/unsafe"
import * as String from "std/string"

struct Leaf {
    name: String
}

struct Node {
    leaf: Leaf
}

enum Slot {
    Full(Node)
    Empty
}

fn build() -> Int {
    let leaf = Leaf { name: String.from("leaf") }
    let node = Node { leaf: leaf }
    let slot: Slot = Full(node)
    return 7
}

fn main() -> Int {
    let before = core.mem_live_bytes()
    let result = build()
    let after = core.mem_live_bytes()
    if result == 7 {
        if after == before {
            return 42
        }
        return 2
    }
    return 1
}
