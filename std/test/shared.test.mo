import * as shared from "std/shared"

test "std shared int clones coordinate one value" {
    let one: shared.Shared<Int> = shared.new_int(10)
    let two: shared.Shared<Int> = shared.clone_int(one)
    assert(shared.get_int(one) == 10)
    assert(shared.set_int(two, 15) == 15)
    assert(shared.get_int(one) == 15)
    assert(shared.get_int(two) == 15)
}
