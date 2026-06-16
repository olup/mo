import * as process from "std/process"
import * as String from "std/string"

test "std process paths" {
    assert(String.len(process.current_dir()) > 0)
    assert(String.len(process.executable_path()) > 0)
}
