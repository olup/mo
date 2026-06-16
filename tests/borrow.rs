use mo::borrow::{check_borrows, BorrowReport};
use mo::hir::lower_program;
use mo::ownership::check_ownership;
use mo::resolve::resolve_program;
use mo::semantics::{check_program, Target};
use mo::typeck::type_check_program;
use mo::{Lexer, Parser};

fn borrow_check(source: &str) -> Result<BorrowReport, Vec<mo::semantics::Diagnostic>> {
    let tokens = Lexer::new(source).tokenize().expect("lex");
    let program = Parser::new(tokens).parse_program().expect("parse");
    let target = Target::macos_aarch64();
    check_program(&program, &target)?;
    let hir = lower_program(&program, &target)?;
    resolve_program(&hir)?;
    type_check_program(&hir)?;
    check_ownership(&hir)?;
    check_borrows(&hir)
}

#[test]
fn shared_borrows_can_overlap() {
    borrow_check(
        r#"
fn main() {
    let value = "Ada"
    let first = &value
    let second = &value
    print(first)
    print(second)
}
"#,
    )
    .expect("borrow check");
}

#[test]
fn value_can_move_after_last_borrow_use() {
    borrow_check(
        r#"
extern "C" {
    fn raw_string_concat(a: Str, b: Str) -> String
}

fn consume(value: String) {
}

fn main() {
    let value: String = raw_string_concat("Ada", "")
    let borrowed = &value
    print(borrowed)
    consume(value)
}
"#,
    )
    .expect("borrow check");
}

#[test]
fn mutable_borrow_excludes_reads_until_last_use() {
    let errors = borrow_check(
        r#"
fn main() {
    let mut value = "Ada"
    let borrowed = &mut value
    print(value)
    print(borrowed)
}
"#,
    )
    .expect_err("expected read while mutably borrowed");

    assert!(errors.iter().any(|error| error
        .message
        .contains("cannot use `value` while mutably borrowed")));
}

#[test]
fn borrowed_value_cannot_be_moved_while_borrow_is_live() {
    let errors = borrow_check(
        r#"
extern "C" {
    fn raw_string_concat(a: Str, b: Str) -> String
}

fn consume(value: String) {
}

fn main() {
    let value: String = raw_string_concat("Ada", "")
    let borrowed = &value
    consume(value)
    print(borrowed)
}
"#,
    )
    .expect_err("expected move while borrowed");

    assert!(errors
        .iter()
        .any(|error| error.message.contains("cannot move `value` while borrowed")));
}

#[test]
fn returning_reference_to_local_errors() {
    let errors = borrow_check(
        r#"
extern "C" {
    fn raw_string_concat(a: Str, b: Str) -> String
}

fn bad() -> &String {
    let value: String = raw_string_concat("Ada", "")
    return &value
}
"#,
    )
    .expect_err("expected local reference escape");

    assert!(errors.iter().any(|error| error
        .message
        .contains("cannot return reference to local `value`")));
}
