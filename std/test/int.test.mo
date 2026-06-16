import * as int from "std/int"

test "std int module test file pattern" {
    assert(int.parse_decimal_or("42", 0) == 42)
    assert(int.to_string(42) == "42")
    assert(int.checked_add_or(int.max_value(), 1, 7) == 7)
}
