// Regression hardening spec: cloned AtomicInt handles coordinate across threads.

import * as atomic from "std/atomic"
import * as thread from "std/thread"

test "atomic clones coordinate concurrent increments without crashing" {
    let counter = atomic.int(0)
    let a = atomic.clone(counter)
    let b = atomic.clone(counter)
    let c = atomic.clone(counter)
    let d = atomic.clone(counter)

    let one = thread.spawn(move fn() {
        let mut index = 0
        while index < 25 {
            atomic.add(a, 1)
            index = index + 1
        }
    })
    let two = thread.spawn(move fn() {
        let mut index = 0
        while index < 25 {
            atomic.add(b, 1)
            index = index + 1
        }
    })
    let three = thread.spawn(move fn() {
        let mut index = 0
        while index < 25 {
            atomic.add(c, 1)
            index = index + 1
        }
    })
    let four = thread.spawn(move fn() {
        let mut index = 0
        while index < 25 {
            atomic.add(d, 1)
            index = index + 1
        }
    })

    assert(thread.join(one) == 0)
    assert(thread.join(two) == 0)
    assert(thread.join(three) == 0)
    assert(thread.join(four) == 0)
    assert(atomic.load(counter) == 100)
    atomic.destroy(counter)
}
