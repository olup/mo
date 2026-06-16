import * as express from "./express"
import * as http from "std/http"
import * as pokemon from "./pokemon"
import { Result } from "std/result"

fn storage_error_response(error: pokemon.StoreError) -> http.Response {
    return http.internal_server_error_response()
}

fn get_pokemon_checked(client: Int, request: &http.Request, store_path: &Str) -> Result<http.Response, pokemon.StoreError> {
    let value = pokemon.read_checked(store_path)?
    let body = pokemon.encode(value)
    let response: http.Response = http.json_response(body)
    return Ok(response)
}

fn post_pokemon_checked(client: Int, request: &http.Request, store_path: &Str) -> Result<http.Response, pokemon.StoreError> {
    let value = pokemon.train_checked(store_path)?
    let body = pokemon.encode(value)
    let response: http.Response = http.created_json_response(body)
    return Ok(response)
}

pub fn get_pokemon(client: Int, request: &http.Request, store_path: &Str) -> http.Response {
    return match get_pokemon_checked(client, request, store_path) {
        Ok(response) => response
        Err(error) => storage_error_response(error)
    }
}

pub fn post_pokemon(client: Int, request: &http.Request, store_path: &Str) -> http.Response {
    return match post_pokemon_checked(client, request, store_path) {
        Ok(response) => response
        Err(error) => storage_error_response(error)
    }
}

pub fn get_health(client: Int, request: &http.Request, store_path: &Str) -> http.Response {
    let response: http.Response = http.json_response("{\"status\":\"ok\"}")
    return response
}

pub fn before_request(client: Int, request: &http.Request, store_path: &Str) -> Int {
    return 0
}

pub fn app(backlog: Int) -> express.App {
    let mut server = express.with_backlog(backlog)
    server.use_before(before_request)
    server.get("/pokemon", get_pokemon)
    server.post("/pokemon", post_pokemon)
    server.get("/health", get_health)
    return server
}
