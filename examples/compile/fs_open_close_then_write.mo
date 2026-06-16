import * as fs from "std/fs"

fn main() -> Int {
    let file = ".mo-open-close-write.tmp"
    fs.write_text(file, "first")
    let fd = fs.open_read(file)
    if fd > 0 {
        fs.close_fd(fd)
    }
    let wrote = fs.write_text(file, "second")
    fs.remove(file)
    if wrote == 6 {
        return 42
    }
    return 1
}
