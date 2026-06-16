import * as String from "std/string"

struct User {
    id: Int
    name: String
}

enum Lookup {
    Found(User)
    Missing
}

fn main() -> Int {
    let user = User { id: 42, name: String.from("Ada") }
    let result: Lookup = Found(user)
    let found = match result {
        Found(value) => value
        Missing => User { id: 0, name: String.from("nobody") }
    }
    print(found.name)
    return found.id
}
