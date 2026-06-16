import * as toml from "../src/toml"

fn assert_parse_ok(text: &Str) {
    let result = toml.parse(text)
    assert(result.ok)
}

test "toml parses package manifest scalars" {
    let doc = "
# package metadata
[package]
name = \"raylib\"
root = \"src\"
enabled = true
retries = 3
"
    assert_parse_ok(doc)

    assert(toml.has(doc, "package.name"))
    assert(toml.has(doc, "package.root"))
    assert(toml.has(doc, "package.enabled"))
    assert(toml.has(doc, "package.retries"))
}

test "toml parses dotted sections and native arrays" {
    let doc = "
[native.macos.aarch64]
static_libraries = [\"vendor/libraylib_mo.a\", \"vendor/libraylib.a\"]
link_args = [
    \"-framework\", \"Cocoa\",
    \"-framework\", \"IOKit\",
    \"-framework\", \"CoreVideo\",
]
"
    assert_parse_ok(doc)

    assert(toml.array_len(doc, "native.macos.aarch64.static_libraries") == 2)
    assert(toml.array_len(doc, "link_args") == 6)
}

test "toml parses scripts and ignores comments outside strings" {
    let doc = "
[scripts]
prepare = \"./prepare-raylib.sh\" # build native archive
build = \"mo build app/main.mo -o /tmp/mo_raylib_3d_demo\"
literal_hash = \"value # not a comment\"
"
    assert_parse_ok(doc)

    assert(toml.has(doc, "scripts.prepare"))
    assert(toml.has(doc, "scripts.build"))
    assert(toml.has(doc, "scripts.literal_hash"))
}

test "toml parses typed arrays" {
    let doc = "
numbers = [1, -2, 3]
flags = [true, false, true]
"
    assert_parse_ok(doc)

    assert(toml.array_len(doc, "numbers") == 3)
    assert(toml.array_int(doc, "numbers", 0) == 1)
    assert(toml.array_int(doc, "numbers", 1) == 0 - 2)
    assert(toml.array_int(doc, "numbers", 3) == 0)
    assert(toml.array_bool(doc, "flags", 0))
    assert(!toml.array_bool(doc, "flags", 1))
}
