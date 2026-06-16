import * as core from "core/unsafe"
import * as String from "std/string"

pub struct File {
    fd: Int
}

extern "C" {
    fn open(path: &Str, flags: Int32, mode: Int32) -> Int32
    fn creat(path: &Str, mode: Int32) -> Int32
    fn read(fd: Int32, buffer: &String, count: Int) -> Int
    fn close(fd: Int32) -> Int32
    fn chmod(path: &Str, mode: Int32) -> Int32
    fn unlink(path: &Str) -> Int32
}

pub fn open_read(path: &Str) -> Int {
    return open(path, 0, 0)
}

pub fn open_write_truncate(path: &Str) -> Int {
    let fd = creat(path, 420)
    if fd > 0 {
        return fd
    }
    chmod(path, 420)
    return creat(path, 420)
}

pub fn close_fd(fd: Int) -> Int {
    return close(fd)
}

pub fn remove(path: &Str) -> Int {
    return unlink(path)
}

pub fn write_text(path: &Str, text: &Str) -> Int {
    let fd = open_write_truncate(path)
    if fd > 0 {
        let written = core.write(fd, text)
        close(fd)
        return written
    }
    return fd
}

pub fn read_text_or(path: &Str, fallback: &Str) -> String {
    let fd = open_read(path)
    if fd > 0 {
        let buffer = core.alloc_string(4096)
        let count = read(fd, buffer, 4095)
        close(fd)
        if count >= 0 {
            core.string_store8(buffer, count, 0)
            return buffer
        }
        return String.from(fallback)
    }
    return String.from(fallback)
}

pub fn read_text(path: &Str) -> String {
    return read_text_or(path, "")
}

pub fn exists(path: &Str) -> Bool {
    let fd = open_read(path)
    if fd > 0 {
        close(fd)
        return true
    }
    return false
}
