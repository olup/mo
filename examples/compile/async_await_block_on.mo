import * as async from "std/async"

async fn load() -> Int {
    return 40
}

async fn add_two() -> Int {
    let value = load().await
    return value + 2
}

fn main() -> Int {
    return async.block_on(add_two())
}
