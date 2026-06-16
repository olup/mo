module examples.web_server

import { Request, Response, Server } from "std/http"

extern "C" {
    fn raw_string_concat(a: Str, b: Str) -> String
}

struct Pokemon {
    id: Int
    name: String
    kind: String
    level: Int
}

fn response_json(pokemon: Pokemon) -> Response {
    Response {}
}

fn server_new() -> Server {
    Server {}
}

async fn get_pokemon(req: Request) -> Result<Response, Error> {
    let pokemon = Pokemon { id: 25, name: raw_string_concat("Pikachu", ""), kind: raw_string_concat("Electric", ""), level: 5 }
    response_json(pokemon)
}

async fn train_pokemon(req: Request) -> Result<Response, Error> {
    let pokemon = Pokemon { id: 25, name: raw_string_concat("Pikachu", ""), kind: raw_string_concat("Electric", ""), level: 6 }
    response_json(pokemon)
}

async fn main() -> Result<(), Error> {
    server_new()
        .workers(thread.cpu_count())
        .get("/pokemon", get_pokemon)
        .post("/pokemon", train_pokemon)
        .listen("127.0.0.1:3000")
        .await
}
