import * as atomic from "std/atomic"
import * as core from "core/unsafe"
import * as sync from "std/sync"

fn main() -> Int {
    let lock = sync.mutex()
    let cell = core.alloc(8)
    sync.lock(lock)
    core.store64(cell, 0, 21)
    sync.unlock(lock)

    let rw = sync.rwlock()
    sync.write_lock(rw)
    core.store64(cell, 0, core.load64(cell, 0) + 20)
    sync.rw_unlock(rw)
    sync.read_lock(rw)
    let protected = core.load64(cell, 0)
    sync.rw_unlock(rw)

    let counter = atomic.int(1)
    atomic.add(counter, protected)
    let value = atomic.load(counter)

    core.free(cell)
    atomic.destroy(counter)
    sync.rw_destroy(rw)
    sync.destroy(lock)

    if value == 42 {
        return 42
    }
    return 1
}
