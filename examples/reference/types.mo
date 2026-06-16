module examples.types

@repr(.c)
struct Point {
    x: Float64
    y: Float64
}

struct User {
    pub id: Int
    name: String
}

enum Option<T> {
    Some(T)
    None
}

enum Message {
    Quit
    Move { x: Int, y: Int }
    Write(String)
}

fn handle(msg: Message) -> Int {
    match msg {
        Quit => 0
        Move { x, y } => x + y
        Write(text) => text.len()
    }
}

test "types parse" {
    let p = Point { x: 1.0, y: 2.0 }
    assert(p.x == 1.0)
}
