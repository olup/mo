struct Config {
    base: Int
    limit: Int
}

fn add(a: Int, b: Int) -> Int {
    return a + b
}

fn main() -> Int {
    let config = Config { base: 12, limit: 6 }
    let mut i = 0
    let mut total = config.base

    while i < config.limit {
        total = add(total, i)
        i += 1
    }

    if total == 27 {
        print(total)
        return 0
    }

    return 1
}
