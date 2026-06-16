module examples.core

import { Reader, Writer } from "std/io"
import * as fs from "std/fs"

const DEFAULT_PORT: Int = 3000
static STARTED: Bool = false

type Handler = async fn(Request) -> Result<Response, Error>

fn add(a: Int, b: Int) -> Int {
    a + b
}

fn bindings() {
    let x = 1
    let mut y = 2
    y += x
}

test "core bindings and functions parse" {
    assert(add(1, 2) == 3)
}
