import * as fs from "std/fs"
import * as path from "std/path"
import * as process from "std/process"

fn fixture() -> String {
    return path.join(process.current_dir(), "build/std_fs_test.txt")
}

test "std fs write read exists remove" {
    let file = fixture()
    assert(fs.write_text(file, "hello") > 0)
    assert(fs.exists(file))
    assert(fs.read_text_or(file, "missing") == "hello")
    assert(fs.remove(file) == 0)
}
