use mo::hir::lower_program;
use mo::package::load_package;
use mo::resolve::resolve_program;
use mo::semantics::{check_program, Target};
use mo::{Lexer, Parser};

fn resolve(source: &str) -> Result<mo::resolve::ResolveReport, Vec<mo::semantics::Diagnostic>> {
    let tokens = Lexer::new(source).tokenize().expect("lex");
    let program = Parser::new(tokens).parse_program().expect("parse");
    let target = Target::macos_aarch64();
    check_program(&program, &target)?;
    let hir = lower_program(&program, &target)?;
    resolve_program(&hir)
}

fn resolve_package(
    path: &std::path::Path,
) -> Result<mo::resolve::ResolveReport, Vec<mo::semantics::Diagnostic>> {
    let program = load_package(path)?;
    let target = Target::macos_aarch64();
    check_program(&program, &target)?;
    let hir = lower_program(&program, &target)?;
    resolve_program(&hir)
}

#[test]
fn resolves_local_and_function_references() {
    let report = resolve(
        r#"
fn add(a: Int, b: Int) -> Int {
    a + b
}

fn main() -> Int {
    let value = add(1, 2)
    return value
}
"#,
    )
    .expect("resolve");

    assert!(report.resolved_value_refs >= 4);
}

#[test]
fn unknown_local_reference_errors() {
    let errors = resolve(
        r#"
fn main() -> Int {
    return missing
}
"#,
    )
    .expect_err("expected unknown name");

    assert!(errors
        .iter()
        .any(|error| error.message.contains("unknown value `missing`")));
}

#[test]
fn disabled_target_function_is_not_resolved() {
    let errors = resolve(
        r#"
@target(.linux) {
    fn platform() -> Int { 1 }
}

fn main() -> Int {
    platform()
}
"#,
    )
    .expect_err("expected target-filtered unknown name");

    assert!(errors
        .iter()
        .any(|error| error.message.contains("unknown value `platform`")));
}

#[test]
fn namespace_alias_resolves_public_members() {
    let dir = std::env::temp_dir().join(format!(
        "mo_resolver_namespace_alias_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp package dir");
    std::fs::write(
        dir.join("pokemon_server.mo"),
        r#"
pub fn answer() -> Int {
    return 42
}
"#,
    )
    .expect("write pokemon server module");
    std::fs::write(
        dir.join("main.mo"),
        r#"
import * as server from "./pokemon_server"

fn main() -> Int {
    return server.answer()
}
"#,
    )
    .expect("write main module");

    let report = resolve_package(&dir.join("main.mo")).expect("resolve package");
    assert!(report.resolved_value_refs >= 1);
}

#[test]
fn namespace_alias_does_not_rewrite_transitive_imports() {
    let dir = std::env::temp_dir().join(format!(
        "mo_resolver_namespace_transitive_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp package dir");
    std::fs::write(
        dir.join("net.mo"),
        r#"
pub fn listen(fd: Int, backlog: Int) -> Int {
    return fd + backlog
}

pub fn open() -> Int {
    return listen(1, 2)
}
"#,
    )
    .expect("write net module");
    std::fs::write(
        dir.join("server.mo"),
        r#"
import * as net from "./net"

pub fn listen(backlog: Int) -> Int {
    return net.open() + backlog
}
"#,
    )
    .expect("write server module");
    std::fs::write(
        dir.join("main.mo"),
        r#"
import * as server from "./server"

fn main() -> Int {
    return server.listen(4)
}
"#,
    )
    .expect("write main module");

    let report = resolve_package(&dir.join("main.mo")).expect("resolve package");
    assert!(report.resolved_value_refs >= 1);
}

#[test]
fn selected_import_carries_private_implementation_dependencies() {
    let dir = std::env::temp_dir().join(format!(
        "mo_resolver_selected_import_deps_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp package dir");
    std::fs::write(
        dir.join("math.mo"),
        r#"
fn add_one(value: Int) -> Int {
    return value + 1
}

pub fn answer() -> Int {
    return add_one(41)
}
"#,
    )
    .expect("write math module");
    std::fs::write(
        dir.join("main.mo"),
        r#"
import { answer } from "./math"

fn main() -> Int {
    return answer()
}
"#,
    )
    .expect("write main module");

    let report = resolve_package(&dir.join("main.mo")).expect("resolve package");
    assert!(report.resolved_value_refs >= 1);
}

#[test]
fn uppercase_namespace_requires_import_rewrite() {
    let errors = resolve(
        r#"
fn main() -> Int {
    return String.len("hello")
}
"#,
    )
    .expect_err("expected missing namespace");

    assert!(errors
        .iter()
        .any(|error| error.message.contains("unknown namespace `String`")));
}

#[test]
fn namespace_alias_rejects_private_members() {
    let dir = std::env::temp_dir().join(format!(
        "mo_resolver_namespace_alias_private_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp package dir");
    std::fs::write(
        dir.join("pokemon_server.mo"),
        r#"
fn hidden() -> Int {
    return 42
}

pub fn visible() -> Int {
    return 1
}
"#,
    )
    .expect("write pokemon server module");
    std::fs::write(
        dir.join("main.mo"),
        r#"
import * as server from "./pokemon_server"

fn main() -> Int {
    return server.hidden()
}
"#,
    )
    .expect("write main module");

    let errors = resolve_package(&dir.join("main.mo")).expect_err("expected private member error");
    assert!(errors.iter().any(|error| {
        error
            .message
            .contains("`hidden` is not exported by namespace `server`")
    }));
    assert!(errors.iter().any(|error| error
        .message
        .contains("available public export(s): `visible`")));
}

#[test]
fn reference_examples_resolve_known_user_names() {
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
        resolve(&source).unwrap_or_else(|errors| panic!("{path}: {errors:#?}"));
    }
}
