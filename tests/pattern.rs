use mo::hir::lower_program;
use mo::resolve::resolve_program;
use mo::semantics::{check_program, Target};
use mo::typeck::type_check_program;
use mo::{Lexer, Parser};

fn type_check(source: &str) -> Result<mo::typeck::TypeReport, Vec<mo::semantics::Diagnostic>> {
    let tokens = Lexer::new(source).tokenize().expect("lex");
    let program = Parser::new(tokens).parse_program().expect("parse");
    let target = Target::macos_aarch64();
    check_program(&program, &target)?;
    let hir = lower_program(&program, &target)?;
    resolve_program(&hir)?;
    type_check_program(&hir)
}

#[test]
fn exhaustive_enum_match_passes() {
    type_check(
        r#"
enum Option<T> {
    Some(T)
    None
}

fn unwrap_or(value: Option<Int>) -> Int {
    match value {
        Some(x) => x
        None => 0
    }
}
"#,
    )
    .expect("type check");
}

#[test]
fn wildcard_makes_enum_match_exhaustive() {
    type_check(
        r#"
enum Option<T> {
    Some(T)
    None
}

fn unwrap_or(value: Option<Int>) -> Int {
    match value {
        Some(x) => x
        _ => 0
    }
}
"#,
    )
    .expect("type check");
}

#[test]
fn missing_enum_variant_errors() {
    let errors = type_check(
        r#"
enum Option<T> {
    Some(T)
    None
}

fn bad(value: Option<Int>) -> Int {
    match value {
        Some(x) => x
    }
}
"#,
    )
    .expect_err("expected exhaustiveness error");

    assert!(errors
        .iter()
        .any(|error| error.message.contains("non-exhaustive match on `Option`")));
}

#[test]
fn unknown_enum_variant_errors() {
    let errors = type_check(
        r#"
enum Option<T> {
    Some(T)
    None
}

fn bad(value: Option<Int>) -> Int {
    match value {
        Nope => 0
        None => 0
    }
}
"#,
    )
    .expect_err("expected unknown variant error");

    assert!(errors.iter().any(|error| error
        .message
        .contains("unknown variant `Nope` for enum `Option`")));
}

#[test]
fn pattern_bindings_are_arm_local() {
    let errors = type_check(
        r#"
enum Option<T> {
    Some(T)
    None
}

fn bad(value: Option<Int>) -> Int {
    match value {
        Some(x) => x
        None => x
    }
}
"#,
    )
    .expect_err("expected unresolved arm binding");

    assert!(errors
        .iter()
        .any(|error| error.message.contains("unknown value `x`")));
}
