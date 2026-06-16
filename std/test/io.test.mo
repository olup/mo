import * as io from "std/io"

test "std io write empty string" {
    assert(io.write_fd(1, "") == 0)
}
