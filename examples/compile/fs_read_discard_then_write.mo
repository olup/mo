import * as fs from "std/fs"

fn main() -> Int {
    fs.write_text("mo_fs_read_discard_then_write.json", "first")
    fs.read_text("mo_fs_read_discard_then_write.json")
    let wrote = fs.write_text("mo_fs_read_discard_then_write.json", "second")
    fs.remove("mo_fs_read_discard_then_write.json")
    if wrote == 6 {
        return 42
    }
    return 1
}
