import * as fs from "std/fs"
import * as process from "std/process"
import * as path from "std/path"

fn main() -> Int {
    let cwd = process.current_dir()
    let file = path.join(cwd, "mo_fs_process_path_smoke.json")
    let written = fs.write_text(file, "{\"count\":1,\"message\":\"hello\"}")
    if written > 0 {
        let text = fs.read_text(file)
        fs.remove(file)
        return 42
    }
    return 1
}
