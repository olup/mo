import * as core from "core/unsafe"
pub interface Reader {
    fn read() -> Result<String, IOError>
}

pub interface Writer {
    fn write(text: &Str) -> Result<Unit, IOError>
}

extern "C" {
    fn read(fd: Int32, buffer: &String, count: Int) -> Int
    fn close(fd: Int32) -> Int32
}

pub fn write_fd(fd: Int, text: &Str) -> Int {
    return core.write(fd, text)
}

pub fn read_byte_fd(fd: Int) -> Int {
    let buffer = core.alloc_string(1)
    let count = read(fd, buffer, 1)
    if count == 1 {
        return core.string_load8(buffer, 0)
    }
    return 0 - 1
}

pub fn close_fd(fd: Int) -> Int {
    return close(fd)
}
