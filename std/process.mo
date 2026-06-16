import * as core from "core/unsafe"
import * as String from "std/string"

extern "C" {
    fn getcwd(buffer: &String, size: Int) -> Str
    fn _NSGetExecutablePath(buffer: &String, size: Int) -> Int32
}

pub fn current_dir() -> String {
    let buffer = core.alloc_string(1024)
    let result = getcwd(buffer, 1024)
    if String.len(result) == 0 {
        return String.from("")
    }
    return buffer
}

pub fn executable_path() -> String {
    let buffer = core.alloc_string(1024)
    let size = core.alloc(4)
    core.store32le(size, 0, 1024)
    let ok = _NSGetExecutablePath(buffer, size)
    core.free(size)
    if ok == 0 {
        return buffer
    }
    return String.from("")
}
