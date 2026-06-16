import * as core from "core/unsafe"
import * as sync from "std/sync"

pub struct AtomicInt {
    mutex: Int
    cell: Int
}

pub fn int(value: Int) -> AtomicInt {
    let lock = sync.mutex()
    let cell = core.alloc(8)
    core.store64(cell, 0, value)
    return AtomicInt { mutex: sync.raw_mutex(lock), cell: cell }
}

pub fn clone(value: &AtomicInt) -> AtomicInt {
    return AtomicInt { mutex: value.mutex, cell: value.cell }
}

pub fn load(value: &AtomicInt) -> Int {
    sync.lock_raw(value.mutex)
    let current = core.load64(value.cell, 0)
    sync.unlock_raw(value.mutex)
    return current
}

pub fn store(value: &AtomicInt, next: Int) -> Int {
    sync.lock_raw(value.mutex)
    core.store64(value.cell, 0, next)
    sync.unlock_raw(value.mutex)
    return next
}

pub fn add(value: &AtomicInt, delta: Int) -> Int {
    sync.lock_raw(value.mutex)
    let next = core.load64(value.cell, 0) + delta
    core.store64(value.cell, 0, next)
    sync.unlock_raw(value.mutex)
    return next
}

pub fn destroy(value: &AtomicInt) -> Int {
    core.free(value.cell)
    return 0
}
