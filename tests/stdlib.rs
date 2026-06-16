use mo::borrow::check_borrows;
use mo::dropck::check_drops;
use mo::hir::lower_program;
use mo::ownership::check_ownership;
use mo::resolve::resolve_program;
use mo::semantics::{check_program, Target};
use mo::typeck::type_check_program;
use mo::{Lexer, Parser};

fn check(source: &str) -> Result<(), Vec<mo::semantics::Diagnostic>> {
    let tokens = Lexer::new(source).tokenize().expect("lex");
    let program = Parser::new(tokens).parse_program().expect("parse");
    let target = Target::macos_aarch64();
    check_program(&program, &target)?;
    let hir = lower_program(&program, &target)?;
    resolve_program(&hir)?;
    type_check_program(&hir)?;
    check_ownership(&hir)?;
    check_borrows(&hir)?;
    check_drops(&hir)?;
    Ok(())
}

#[test]
fn prelude_core_names_resolve() {
    check(
        r#"
fn main(
    a: Option<String>,
    b: Result<String, Error>,
    c: Vec<Int>,
    d: Box<String>,
    e: Slice<Byte>,
    f: Display,
) {
    print("ok")
}
"#,
    )
    .expect("stdlib names resolve");
}

#[test]
fn json_is_not_a_prelude_or_core_name() {
    let errors = check(
        r#"
fn main(value: JsonValue) {
    json.int_value(1)
}
"#,
    )
    .expect_err("json should not be a compiler-known std/prelude name");

    assert!(errors.iter().any(|error| {
        error.message.contains("unknown type `JsonValue`")
            || error.message.contains("unknown value `json`")
    }));
}

#[test]
fn raw_intrinsics_are_not_prelude_values() {
    let errors = check(
        r#"
fn main() -> Int {
    return raw_write(1, "nope")
}
"#,
    )
    .expect_err("raw intrinsics should not be prelude values");

    assert!(errors
        .iter()
        .any(|error| error.message.contains("unknown value `raw_write`")));
}

#[test]
fn result_try_type_checks() {
    check(
        r#"
enum Result<T, E> {
    Ok(T)
    Err(E)
}

fn source() -> Result<String, Error> {
}

fn main() -> Result<String, Error> {
    let value = source()?
    return Ok(value)
}
"#,
    )
    .expect("result try checks");
}

#[test]
fn std_option_and_result_modules_type_check() {
    check(
        r#"
import { Option } from "std/option"
import { Result } from "std/result"

fn maybe() -> Option<Int> {
    return Some(41)
}

fn parse() -> Result<Int, Str> {
    return Ok(1)
}

fn main() -> Result<Int, Str> {
    let value = maybe()
    let parsed = parse()?
    let unwrapped = match value {
        Some(item) => item
        None => 0
    }
    return Ok(unwrapped + parsed)
}
"#,
    )
    .expect("std Option and Result modules should type-check");
}

#[test]
fn option_match_type_checks() {
    check(
        r#"
fn main(value: Option<String>) {
    match value {
        Some(name) => print(name)
        None => print("none")
    }
}
"#,
    )
    .expect("option match checks");
}
