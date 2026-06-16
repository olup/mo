import * as String from "std/string"

pub fn separator() -> String {
    return String.from("/")
}

pub fn join(base: &Str, child: &Str) -> String {
    if String.len(base) == 0 {
        return String.from(child)
    }
    if String.len(child) == 0 {
        return String.from(base)
    }
    return String.concat(String.concat(base, "/"), child)
}
