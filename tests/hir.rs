use mo::hir::{lower_program, HirItemKind};
use mo::package::load_package;
use mo::semantics::Target;
use mo::{Lexer, Parser};

fn lower(source: &str) -> mo::hir::HirProgram {
    let tokens = Lexer::new(source).tokenize().expect("lex");
    let program = Parser::new(tokens).parse_program().expect("parse");
    lower_program(&program, &Target::macos_aarch64()).expect("lower")
}

#[test]
fn lowers_enabled_target_items_only() {
    let hir = lower(
        r#"
@target(.macos) {
    fn platform() -> String { "macos" }
}

@target(.linux) {
    fn platform() -> String { "linux" }
}

fn always() {}
"#,
    );

    let names: Vec<_> = hir.items.iter().map(|item| item.name.as_str()).collect();
    assert_eq!(names, vec!["platform", "always"]);
    assert_eq!(hir.items[0].id.0, 0);
    assert_eq!(hir.items[1].id.0, 1);
}

#[test]
fn lowers_item_kinds_and_function_bodies() {
    let hir = lower(
        r#"
struct User {
    name: String
}

fn greet(name: String) -> String {
    let msg = name
    return msg
}

test "greet parses" {
    greet("Ada")
}
"#,
    );

    assert!(matches!(hir.items[0].kind, HirItemKind::Struct));
    assert!(matches!(hir.items[1].kind, HirItemKind::Function));
    assert!(matches!(hir.items[2].kind, HirItemKind::Test));

    let function = hir
        .functions
        .iter()
        .find(|function| function.name == "greet")
        .expect("function");
    assert_eq!(function.params.len(), 1);
    assert_eq!(function.body.statements.len(), 2);

    let test = hir
        .tests
        .iter()
        .find(|test| test.name == "greet parses")
        .expect("test");
    assert_eq!(test.body.statements.len(), 1);
}

#[test]
fn lowers_module_ids_from_module_declarations() {
    let hir = lower(
        r#"
module app.main

struct User {
    name: String
}

fn greet() {}
"#,
    );

    let module = hir
        .modules
        .iter()
        .find(|module| module.path == vec!["app".to_string(), "main".to_string()])
        .expect("app.main module");
    assert!(hir.items.iter().all(|item| item.module == module.id));
    assert_eq!(hir.structs[0].module, module.id);
    assert_eq!(hir.functions[0].module, module.id);
}

#[test]
fn package_imports_have_distinct_module_ids() {
    let dir = std::env::temp_dir().join(format!("mo_hir_modules_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp package dir");
    std::fs::write(
        dir.join("math.mo"),
        r#"
pub fn add(a: Int, b: Int) -> Int {
    return a + b
}
"#,
    )
    .expect("write math module");
    std::fs::write(
        dir.join("main.mo"),
        r#"
import { add } from "./math"

fn main() -> Int {
    return add(20, 22)
}
"#,
    )
    .expect("write main module");

    let program = load_package(&dir.join("main.mo")).expect("load package");
    let hir = lower_program(&program, &Target::macos_aarch64()).expect("lower");
    let add = hir
        .functions
        .iter()
        .find(|function| function.name == "add")
        .expect("add");
    let main = hir
        .functions
        .iter()
        .find(|function| function.name == "main")
        .expect("main");

    assert_ne!(add.module, main.module);
    assert!(hir.modules.iter().any(|module| module.id == add.module));
    assert!(hir.modules.iter().any(|module| module.id == main.module));
}

#[test]
fn lowers_reference_examples() {
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
        let hir = lower(&source);
        assert!(!hir.items.is_empty(), "{path} should lower items");
    }
}
