import * as core from "core/unsafe"
import * as String from "std/string"
import * as sync from "std/sync"

pub struct Channel<T> {
    pub mutex: Int
    pub not_empty: Int
    pub not_full: Int
    pub cell: Int
}

pub type IntChannel = Channel<Int>

pub fn new<T>() -> Channel<T>
pub fn clone<T>(ch: &Channel<T>) -> Channel<T>
pub fn send<T>(ch: &Channel<T>, value: T) -> Int
pub fn recv<T>(ch: &Channel<T>) -> T
pub fn close<T>(ch: &Channel<T>) -> Int
pub fn destroy<T>(ch: &Channel<T>) -> Int

extern "C" {
    fn pthread_cond_init(cond: Int, attr: Int) -> Int32
    fn pthread_cond_wait(cond: Int, mutex: Int) -> Int32
    fn pthread_cond_signal(cond: Int) -> Int32
    fn pthread_cond_broadcast(cond: Int) -> Int32
    fn pthread_cond_destroy(cond: Int) -> Int32
}

fn cond() -> Int {
    let raw = core.alloc(64)
    pthread_cond_init(raw, 0)
    return raw
}

pub fn new_int() -> Channel<Int> {
    let lock = sync.mutex()
    let cell = core.alloc(24)
    core.store64(cell, 0, 0)
    core.store64(cell, 8, 0)
    core.store64(cell, 16, 0)
    return Channel {
        mutex: sync.raw_mutex(lock),
        not_empty: cond(),
        not_full: cond(),
        cell: cell
    }
}

pub fn new_bool() -> Channel<Bool> {
    let lock = sync.mutex()
    let cell = core.alloc(24)
    core.store64(cell, 0, 0)
    core.store64(cell, 8, 0)
    core.store64(cell, 16, 0)
    return Channel {
        mutex: sync.raw_mutex(lock),
        not_empty: cond(),
        not_full: cond(),
        cell: cell
    }
}

pub fn new_string() -> Channel<String> {
    let lock = sync.mutex()
    let cell = core.alloc(24)
    core.store64(cell, 0, 0)
    core.store64(cell, 8, 0)
    core.store64(cell, 16, 0)
    return Channel {
        mutex: sync.raw_mutex(lock),
        not_empty: cond(),
        not_full: cond(),
        cell: cell
    }
}

pub fn new_function() -> Channel<fn() -> ()> {
    let lock = sync.mutex()
    let cell = core.alloc(24)
    core.store64(cell, 0, 0)
    core.store64(cell, 8, 0)
    core.store64(cell, 16, 0)
    return Channel {
        mutex: sync.raw_mutex(lock),
        not_empty: cond(),
        not_full: cond(),
        cell: cell
    }
}

pub fn int() -> Channel<Int> {
    return new_int()
}

pub fn string() -> Channel<String> {
    return new_string()
}

pub fn clone_int(ch: &Channel<Int>) -> Channel<Int> {
    return Channel {
        mutex: ch.mutex,
        not_empty: ch.not_empty,
        not_full: ch.not_full,
        cell: ch.cell
    }
}

pub fn clone_bool(ch: &Channel<Bool>) -> Channel<Bool> {
    return Channel {
        mutex: ch.mutex,
        not_empty: ch.not_empty,
        not_full: ch.not_full,
        cell: ch.cell
    }
}

pub fn clone_string(ch: &Channel<String>) -> Channel<String> {
    return Channel {
        mutex: ch.mutex,
        not_empty: ch.not_empty,
        not_full: ch.not_full,
        cell: ch.cell
    }
}

pub fn clone_function(ch: &Channel<fn() -> ()>) -> Channel<fn() -> ()> {
    return Channel {
        mutex: ch.mutex,
        not_empty: ch.not_empty,
        not_full: ch.not_full,
        cell: ch.cell
    }
}

pub fn send_int(ch: &Channel<Int>, value: Int) -> Int {
    sync.lock_raw(ch.mutex)
    while core.load64(ch.cell, 8) > 0 {
        pthread_cond_wait(ch.not_full, ch.mutex)
    }
    if core.load64(ch.cell, 16) != 0 {
        sync.unlock_raw(ch.mutex)
        return 0 - 1
    }
    core.store64(ch.cell, 0, value)
    core.store64(ch.cell, 8, 1)
    pthread_cond_signal(ch.not_empty)
    sync.unlock_raw(ch.mutex)
    return 0
}

pub fn send_bool(ch: &Channel<Bool>, value: Bool) -> Int {
    sync.lock_raw(ch.mutex)
    while core.load64(ch.cell, 8) > 0 {
        pthread_cond_wait(ch.not_full, ch.mutex)
    }
    if core.load64(ch.cell, 16) != 0 {
        sync.unlock_raw(ch.mutex)
        return 0 - 1
    }
    if value {
        core.store64(ch.cell, 0, 1)
    } else {
        core.store64(ch.cell, 0, 0)
    }
    core.store64(ch.cell, 8, 1)
    pthread_cond_signal(ch.not_empty)
    sync.unlock_raw(ch.mutex)
    return 0
}

pub fn send_string_ref(ch: &Channel<String>, value: &Str) -> Int {
    sync.lock_raw(ch.mutex)
    while core.load64(ch.cell, 8) > 0 {
        pthread_cond_wait(ch.not_full, ch.mutex)
    }
    if core.load64(ch.cell, 16) != 0 {
        sync.unlock_raw(ch.mutex)
        return 0 - 1
    }
    core.store64(ch.cell, 0, core.string_clone_ptr(value))
    core.store64(ch.cell, 8, 1)
    pthread_cond_signal(ch.not_empty)
    sync.unlock_raw(ch.mutex)
    return 0
}

pub fn send_function(ch: &Channel<fn() -> ()>, value: fn() -> ()) -> Int {
    sync.lock_raw(ch.mutex)
    while core.load64(ch.cell, 8) > 0 {
        pthread_cond_wait(ch.not_full, ch.mutex)
    }
    if core.load64(ch.cell, 16) != 0 {
        sync.unlock_raw(ch.mutex)
        return 0 - 1
    }
    core.store64(ch.cell, 0, core.function_ptr(value))
    core.store64(ch.cell, 8, 1)
    pthread_cond_signal(ch.not_empty)
    sync.unlock_raw(ch.mutex)
    return 0
}

pub fn recv_int(ch: &Channel<Int>) -> Int {
    sync.lock_raw(ch.mutex)
    while core.load64(ch.cell, 8) == 0 {
        if core.load64(ch.cell, 16) != 0 {
            sync.unlock_raw(ch.mutex)
            return 0 - 1
        }
        pthread_cond_wait(ch.not_empty, ch.mutex)
    }
    let value = core.load64(ch.cell, 0)
    core.store64(ch.cell, 8, 0)
    pthread_cond_signal(ch.not_full)
    sync.unlock_raw(ch.mutex)
    return value
}

pub fn recv_bool(ch: &Channel<Bool>) -> Bool {
    sync.lock_raw(ch.mutex)
    while core.load64(ch.cell, 8) == 0 {
        if core.load64(ch.cell, 16) != 0 {
            sync.unlock_raw(ch.mutex)
            return false
        }
        pthread_cond_wait(ch.not_empty, ch.mutex)
    }
    let value = core.load64(ch.cell, 0)
    core.store64(ch.cell, 8, 0)
    pthread_cond_signal(ch.not_full)
    sync.unlock_raw(ch.mutex)
    return value != 0
}

pub fn send_string(ch: &Channel<String>, value: String) -> Int {
    return send_string_ref(ch, value)
}

pub fn recv_string(ch: &Channel<String>) -> String {
    sync.lock_raw(ch.mutex)
    while core.load64(ch.cell, 8) == 0 {
        if core.load64(ch.cell, 16) != 0 {
            sync.unlock_raw(ch.mutex)
            return String.from("")
        }
        pthread_cond_wait(ch.not_empty, ch.mutex)
    }
    let ptr = core.load64(ch.cell, 0)
    core.store64(ch.cell, 8, 0)
    pthread_cond_signal(ch.not_full)
    sync.unlock_raw(ch.mutex)
    return core.string_from_ptr(ptr)
}

pub fn recv_function(ch: &Channel<fn() -> ()>) -> fn() -> () {
    sync.lock_raw(ch.mutex)
    while core.load64(ch.cell, 8) == 0 {
        if core.load64(ch.cell, 16) != 0 {
            sync.unlock_raw(ch.mutex)
            return core.function_from_ptr(0)
        }
        pthread_cond_wait(ch.not_empty, ch.mutex)
    }
    let ptr = core.load64(ch.cell, 0)
    core.store64(ch.cell, 8, 0)
    pthread_cond_signal(ch.not_full)
    sync.unlock_raw(ch.mutex)
    return core.function_from_ptr(ptr)
}

pub fn close_int(ch: &Channel<Int>) -> Int {
    sync.lock_raw(ch.mutex)
    core.store64(ch.cell, 16, 1)
    pthread_cond_broadcast(ch.not_empty)
    pthread_cond_broadcast(ch.not_full)
    sync.unlock_raw(ch.mutex)
    return 0
}

pub fn close_bool(ch: &Channel<Bool>) -> Int {
    sync.lock_raw(ch.mutex)
    core.store64(ch.cell, 16, 1)
    pthread_cond_broadcast(ch.not_empty)
    pthread_cond_broadcast(ch.not_full)
    sync.unlock_raw(ch.mutex)
    return 0
}

pub fn close_string(ch: &Channel<String>) -> Int {
    sync.lock_raw(ch.mutex)
    core.store64(ch.cell, 16, 1)
    pthread_cond_broadcast(ch.not_empty)
    pthread_cond_broadcast(ch.not_full)
    sync.unlock_raw(ch.mutex)
    return 0
}

pub fn close_function(ch: &Channel<fn() -> ()>) -> Int {
    sync.lock_raw(ch.mutex)
    core.store64(ch.cell, 16, 1)
    pthread_cond_broadcast(ch.not_empty)
    pthread_cond_broadcast(ch.not_full)
    sync.unlock_raw(ch.mutex)
    return 0
}

pub fn destroy_int(ch: &Channel<Int>) -> Int {
    let result1 = pthread_cond_destroy(ch.not_empty)
    let result2 = pthread_cond_destroy(ch.not_full)
    let result3 = sync.destroy_raw_mutex(ch.mutex)
    core.free(ch.not_empty)
    core.free(ch.not_full)
    core.free(ch.cell)
    if result1 != 0 {
        return result1
    }
    if result2 != 0 {
        return result2
    }
    return result3
}

pub fn destroy_function(ch: &Channel<fn() -> ()>) -> Int {
    let result1 = pthread_cond_destroy(ch.not_empty)
    let result2 = pthread_cond_destroy(ch.not_full)
    let result3 = sync.destroy_raw_mutex(ch.mutex)
    core.free(ch.not_empty)
    core.free(ch.not_full)
    core.free(ch.cell)
    if result1 != 0 {
        return result1
    }
    if result2 != 0 {
        return result2
    }
    return result3
}

pub fn destroy_bool(ch: &Channel<Bool>) -> Int {
    let result1 = pthread_cond_destroy(ch.not_empty)
    let result2 = pthread_cond_destroy(ch.not_full)
    let result3 = sync.destroy_raw_mutex(ch.mutex)
    core.free(ch.not_empty)
    core.free(ch.not_full)
    core.free(ch.cell)
    if result1 != 0 {
        return result1
    }
    if result2 != 0 {
        return result2
    }
    return result3
}

pub fn destroy_string(ch: &Channel<String>) -> Int {
    let result1 = pthread_cond_destroy(ch.not_empty)
    let result2 = pthread_cond_destroy(ch.not_full)
    let result3 = sync.destroy_raw_mutex(ch.mutex)
    core.free(ch.not_empty)
    core.free(ch.not_full)
    core.free(ch.cell)
    if result1 != 0 {
        return result1
    }
    if result2 != 0 {
        return result2
    }
    return result3
}
