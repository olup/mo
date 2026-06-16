import * as slice from "std/slice"


test "std byte slice views borrowed string bytes" {
    let whole = slice.from_str("abcd")
    assert(slice.len(whole) == 4)
    assert(!slice.is_empty(whole))
    assert(whole[0] == 97)
    assert(whole[3] == 100)
    assert(whole[4] == 0 - 1)
    let mid = slice.subslice(whole, 1, 2)
    assert(slice.len(mid) == 2)
    assert(mid[0] == 98)
    assert(mid[1] == 99)
}


test "std byte slice clamps invalid ranges" {
    let whole = slice.from_str("abc")
    let tail = slice.subslice(whole, 2, 9)
    assert(slice.len(tail) == 1)
    assert(tail[0] == 99)
    let empty = slice.subslice(whole, 9, 2)
    assert(slice.is_empty(empty))
    assert(empty[0] == 0 - 1)
    let negative = slice.subslice(whole, 1, 0 - 1)
    assert(slice.is_empty(negative))
}
