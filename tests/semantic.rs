use mo::semantics::{check_program, Target};
use mo::{Lexer, Parser};

fn parse(source: &str) -> mo::ast::Program {
    let tokens = Lexer::new(source).tokenize().expect("lex");
    Parser::new(tokens).parse_program().expect("parse")
}

fn check(source: &str) -> Result<mo::semantics::CheckReport, Vec<mo::semantics::Diagnostic>> {
    let program = parse(source);
    check_program(&program, &Target::macos_aarch64())
}

#[test]
fn target_filtering_enables_macos_and_ignores_linux() {
    let report = check(
        r#"
@target(.macos) {
    fn platform() -> String { "macos" }
}

@target(.linux) {
    fn platform() -> String { "linux" }
}
"#,
    )
    .expect("semantic check");

    assert_eq!(report.enabled_symbols, vec!["platform"]);
}

#[test]
fn duplicate_enabled_top_level_names_are_errors() {
    let errors = check(
        r#"
fn same() {}
fn same() {}
"#,
    )
    .expect_err("expected duplicate error");

    assert!(errors
        .iter()
        .any(|error| error.message.contains("duplicate top-level symbol `same`")));
}

#[test]
fn duplicate_names_in_disabled_target_are_ignored() {
    let report = check(
        r#"
fn same() {}

@target(.linux) {
    fn same() {}
}
"#,
    )
    .expect("semantic check");

    assert_eq!(report.enabled_symbols, vec!["same"]);
}

#[test]
fn duplicate_local_let_bindings_are_errors() {
    let errors = check(
        r#"
fn bad() {
    let value = 1
    let value = 2
}
"#,
    )
    .expect_err("expected duplicate local error");

    assert!(errors
        .iter()
        .any(|error| error.message.contains("duplicate local binding `value`")));
}

#[test]
fn all_reference_examples_semantically_check() {
    for path in [
        "examples/reference/core.mo",
        "examples/reference/types.mo",
        "examples/reference/methods_interfaces.mo",
        "examples/reference/memory_errors.mo",
        "examples/reference/closures_async_threads.mo",
        "examples/reference/platform.mo",
        "examples/reference/web_server.mo",
    ] {
        let source = std::fs::read_to_string(path).expect(path);
        check(&source).unwrap_or_else(|errors| panic!("{path}: {errors:#?}"));
    }
}
