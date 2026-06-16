import * as async from "std/async"

fn no_op() {}

test "std async spawn join" {
    let task = async.spawn(no_op)
    assert(async.join(task) == 0)
}
