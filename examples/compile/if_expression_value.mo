fn choose(flag: Bool) -> Int {
    let value = if flag {
        41
    } else {
        1
    }
    return value + 1
}

fn main() -> Int {
    if choose(true) != 42 {
        return 1
    }
    if choose(false) != 2 {
        return 2
    }
    return 42
}
