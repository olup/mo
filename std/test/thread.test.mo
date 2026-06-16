import * as thread from "std/thread"

fn no_op() {}

test "std thread spawn join" {
    let handle = thread.spawn(no_op)
    assert(thread.join(handle) == 0)
}
