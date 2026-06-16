module examples.closures_async_threads

import * as thread from "std/thread"
import * as async from "std/async"

struct User {
    name: String
}

fn response_text(text: &Str) -> Response {
    Response {}
}

fn closures() {
    let prefix = "user:"
    let label = fn(id: Int) -> String {
        prefix + id.to_string()
    }

    let message = "hello"
    thread.spawn(move fn() {
        print(message)
    })
}

async fn fetch_user(id: Int) -> Result<User, Error> {
    let response = http.get("/users/{id}").await?
    response.json<User>().await
}

async fn tasks() -> Result<(), Error> {
    let handler = async fn(req: Request) -> Result<Response, Error> {
        response_text("ok")
    }

    let task = async.spawn(fetch_user(1))
    let user = task.await?
    print(user.name)
    Ok(())
}

test "closures async and threads parse" {
    closures()
}
