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
fn complete_interface_impl_passes() {
    type_check(
        r#"
interface Display {
    fn display(&self) -> String
}

struct User: Display {
    name: String

    fn display(&self) -> String {
        self.name
    }
}
"#,
    )
    .expect("type check");
}

#[test]
fn missing_interface_method_errors() {
    let errors = type_check(
        r#"
interface Display {
    fn display(&self) -> String
}

struct User: Display {
    name: String
}
"#,
    )
    .expect_err("expected missing method");

    assert!(errors.iter().any(|error| error
        .message
        .contains("missing method `display` for interface `Display`")));
}

#[test]
fn wrong_interface_method_return_errors() {
    let errors = type_check(
        r#"
interface Display {
    fn display(&self) -> String
}

struct User: Display {
    name: String

    fn display(&self) -> Int {
        1
    }
}
"#,
    )
    .expect_err("expected signature mismatch");

    assert!(errors.iter().any(|error| error
        .message
        .contains("method `display` return type mismatch")));
}

#[test]
fn impl_interface_parameter_type_is_accepted() {
    type_check(
        r#"
interface Display {
    fn display(&self) -> String
}

struct User: Display {
    name: String

    fn display(&self) -> String {
        self.name
    }
}

fn label(user: &User) -> String {
    user.display()
}
"#,
    )
    .expect("type check");
}
