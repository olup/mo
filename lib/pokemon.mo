import * as core from "core/unsafe"
import * as fs from "std/fs"
import * as json from "./json"
import { Result } from "std/result"
import * as String from "std/string"

pub struct Pokemon {
    pub id: Int
    pub name: String
    pub kind: String
    pub level: Int
}

pub enum StoreError {
    Missing(Int)
    WriteFailed(Int)
}

pub fn starter() -> Pokemon {
    return Pokemon { id: 25, name: String.from("Pikachu"), kind: String.from("Electric"), level: 5 }
}

fn free_owned(value: &String) {
    core.free(core.string_ptr(value))
}

pub fn encode(value: &Pokemon) -> String {
    let id = json.field_int("id", value.id)
    let name = json.field_string("name", value.name)
    let kind = json.field_string("kind", value.kind)
    let level = json.field_int("level", value.level)
    let fields = json.append_field(json.append_field(json.append_field(id, name), kind), level)
    return json.object(fields)
}

pub fn parse_or(text: &Str, fallback: &Pokemon) -> Pokemon {
    let id = json.parse_field_int_or(text, "id", fallback.id)
    let name = json.parse_field_string_or(text, "name", fallback.name)
    let kind = json.parse_field_string_or(text, "kind", fallback.kind)
    let level = json.parse_field_int_or(text, "level", fallback.level)
    return Pokemon { id: id, name: name, kind: kind, level: level }
}

pub fn read_checked(path: &Str) -> Result<Pokemon, StoreError> {
    if fs.exists(path) {
        let text = fs.read_text(path)
        let fallback = starter()
        return Ok(parse_or(text, fallback))
    }
    return Err(Missing(0))
}

pub fn write_checked(path: &Str, value: &Pokemon) -> Result<Int, StoreError> {
    let text = encode(value)
    let written = fs.write_text(path, text)
    free_owned(text)
    if written > 0 {
        return Ok(written)
    }
    return Err(WriteFailed(written))
}

pub fn reset_checked(path: &Str) -> Result<Pokemon, StoreError> {
    let value = starter()
    let written = write_checked(path, value)?
    return Ok(value)
}

pub fn train_checked(path: &Str) -> Result<Pokemon, StoreError> {
    let current = read_checked(path)?
    let next = Pokemon {
        id: current.id,
        name: current.name,
        kind: current.kind,
        level: current.level + 1
    }
    let written = write_checked(path, next)?
    return Ok(next)
}

pub fn read(path: &Str) -> Pokemon {
    return match read_checked(path) {
        Ok(value) => value
        Err(error) => starter()
    }
}

pub fn write(path: &Str, value: &Pokemon) -> Int {
    let written = write_checked(path, value)
    return match written {
        Ok(count) => count
        Err(error) => 0 - 1
    }
}

pub fn reset(path: &Str) -> Pokemon {
    return match reset_checked(path) {
        Ok(value) => value
        Err(error) => starter()
    }
}

pub fn train(path: &Str) -> Pokemon {
    return match train_checked(path) {
        Ok(value) => value
        Err(error) => reset(path)
    }
}
