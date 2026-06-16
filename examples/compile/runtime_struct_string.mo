import * as String from "std/string"

struct User {
    id: Int
    name: String
}

fn make_user(id: Int, name: String) -> User {
    return User { id: id, name: name }
}

fn user_id(user: User) -> Int {
    return user.id
}

fn main() -> Int {
    let user = make_user(42, String.from("Ada"))
    print(user.name)
    return user_id(user)
}
