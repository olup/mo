import * as slice from "std/slice"

fn main() -> Int {
    let whole = slice.from_str("mo slice")
    let view = slice.subslice(whole, 3, 5)
    if slice.len(view) == 5 {
        if view[0] == 115 {
            if view[4] == 101 {
                if view[5] == 0 - 1 {
                    return 42
                }
            }
        }
    }
    return 1
}
