import * as atomic from "std/atomic"

test "std atomic int load store add" {
    let value = atomic.int(40)
    assert(atomic.load(value) == 40)
    assert(atomic.store(value, 41) == 41)
    assert(atomic.add(value, 1) == 42)
    assert(atomic.load(value) == 42)
    assert(atomic.destroy(value) == 0)
}
