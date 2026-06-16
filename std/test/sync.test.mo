import * as sync from "std/sync"

test "std sync mutex lock unlock destroy" {
    let mutex = sync.mutex()
    assert(sync.lock(mutex) == 0)
    assert(sync.unlock(mutex) == 0)
    assert(sync.destroy(mutex) == 0)
}
