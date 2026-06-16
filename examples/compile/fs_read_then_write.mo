import * as fs from "std/fs"
import * as path from "std/path"
import * as process from "std/process"

fn main() -> Int {
    let cwd = process.current_dir()
    let file = path.join(cwd, "mo_fs_read_then_write.json")
    fs.write_text(file, "first")
    let text = fs.read_text(file)
    let wrote = fs.write_text(file, "second")
    fs.remove(file)
    if wrote == 6 {
        return 42
    }
    return 1
}
