import * as core from "core/unsafe"

pub struct Mutex {
    raw: Int
}

pub struct RwLock {
    raw: Int
}

extern "C" {
    fn pthread_mutex_init(mutex: Int, attr: Int) -> Int32
    fn pthread_mutex_lock(mutex: Int) -> Int32
    fn pthread_mutex_unlock(mutex: Int) -> Int32
    fn pthread_mutex_destroy(mutex: Int) -> Int32
    fn pthread_rwlock_init(lock: Int, attr: Int) -> Int32
    fn pthread_rwlock_rdlock(lock: Int) -> Int32
    fn pthread_rwlock_wrlock(lock: Int) -> Int32
    fn pthread_rwlock_unlock(lock: Int) -> Int32
    fn pthread_rwlock_destroy(lock: Int) -> Int32
}

pub fn mutex() -> Mutex {
    let raw = core.alloc(64)
    pthread_mutex_init(raw, 0)
    return Mutex { raw: raw }
}

pub fn raw_mutex(mu: &Mutex) -> Int {
    return mu.raw
}

pub fn lock(mu: &Mutex) -> Int {
    return pthread_mutex_lock(mu.raw)
}

pub fn unlock(mu: &Mutex) -> Int {
    return pthread_mutex_unlock(mu.raw)
}

pub fn destroy(mu: &Mutex) -> Int {
    let result = pthread_mutex_destroy(mu.raw)
    core.free(mu.raw)
    return result
}

pub fn lock_raw(raw: Int) -> Int {
    return pthread_mutex_lock(raw)
}

pub fn unlock_raw(raw: Int) -> Int {
    return pthread_mutex_unlock(raw)
}

pub fn destroy_raw_mutex(raw: Int) -> Int {
    let result = pthread_mutex_destroy(raw)
    core.free(raw)
    return result
}

pub fn rwlock() -> RwLock {
    let raw = core.alloc(256)
    pthread_rwlock_init(raw, 0)
    return RwLock { raw: raw }
}

pub fn read_lock(rw: &RwLock) -> Int {
    return pthread_rwlock_rdlock(rw.raw)
}

pub fn write_lock(rw: &RwLock) -> Int {
    return pthread_rwlock_wrlock(rw.raw)
}

pub fn rw_unlock(rw: &RwLock) -> Int {
    return pthread_rwlock_unlock(rw.raw)
}

pub fn rw_destroy(rw: &RwLock) -> Int {
    let result = pthread_rwlock_destroy(rw.raw)
    core.free(rw.raw)
    return result
}
