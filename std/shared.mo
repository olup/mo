import * as core from "core/unsafe"
import * as sync from "std/sync"

pub struct Shared<T> {
    mutex: Int
    refs: Int
    value: Int
}

pub fn new<T>(value: T) -> Shared<T>
pub fn clone<T>(value: &Shared<T>) -> Shared<T>
pub fn destroy<T>(value: &Shared<T>) -> Int

pub fn new_int(value: Int) -> Shared<Int> {
    let lock = sync.mutex()
    let refs = core.alloc(8)
    let cell = core.alloc(8)
    core.store64(refs, 0, 1)
    core.store64(cell, 0, value)
    return Shared { mutex: sync.raw_mutex(lock), refs: refs, value: cell }
}

pub fn clone_int(value: &Shared<Int>) -> Shared<Int> {
    sync.lock_raw(value.mutex)
    core.store64(value.refs, 0, core.load64(value.refs, 0) + 1)
    sync.unlock_raw(value.mutex)
    return Shared { mutex: value.mutex, refs: value.refs, value: value.value }
}

pub fn get_int(value: &Shared<Int>) -> Int {
    sync.lock_raw(value.mutex)
    let current = core.load64(value.value, 0)
    sync.unlock_raw(value.mutex)
    return current
}

pub fn set_int(value: &Shared<Int>, next: Int) -> Int {
    sync.lock_raw(value.mutex)
    core.store64(value.value, 0, next)
    sync.unlock_raw(value.mutex)
    return next
}

pub fn destroy_int(value: &Shared<Int>) -> Int {
    sync.lock_raw(value.mutex)
    let next = core.load64(value.refs, 0) - 1
    core.store64(value.refs, 0, next)
    sync.unlock_raw(value.mutex)
    if next == 0 {
        core.free(value.refs)
        core.free(value.value)
        sync.destroy_raw_mutex(value.mutex)
    }
    return next
}
