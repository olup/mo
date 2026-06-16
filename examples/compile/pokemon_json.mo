import * as pokemon from "lib/pokemon"

fn main() -> Int {
    let pikachu = pokemon.starter()
    let parsed = pokemon.parse_or(pokemon.encode(pikachu), pikachu)
    if parsed.level == 5 {
        return 42
    }
    return 1
}
