use std::fs;
use std::path::Path;

use mo::ast::{Expr, Item, StmtData, TypeExpr};
use mo::{Lexer, Parser};

fn parse(source: &str) -> mo::ast::Program {
    let tokens = Lexer::new(source).tokenize().expect("lex");
    Parser::new(tokens).parse_program().expect("parse")
}

fn parse_err(source: &str) -> String {
    let tokens = Lexer::new(source).tokenize().expect("lex");
    Parser::new(tokens)
        .parse_program()
        .expect_err("expected parse error")
        .message
}

fn parse_file(path: impl AsRef<Path>) -> mo::ast::Program {
    let path = path.as_ref();
    let source = fs::read_to_string(path).unwrap_or_else(|err| panic!("{}: {err}", path.display()));
    parse(&source)
}

#[test]
fn parses_web_server_shape() {
    let program = parse(
        r#"
import { Request, Response, Server } from "std/http"
import * as json from "json"
import * as sse from "std/sse"

struct User {
    id: Int
    name: String
}

async fn get_user(req: Request) -> Result<Response, Error> {
    let id = req.param("id")?.parse<Int>()?
    let user = User { id, name: "Ada" }
    Response.json(user)
}

async fn main() -> Result<(), Error> {
    Server.new()
        .workers(thread.cpu_count())
        .get("/users/:id", get_user)
        .listen("127.0.0.1:3000")
        .await
}
"#,
    );

    assert_eq!(program.items.len(), 6);
}

#[test]
fn parses_target_and_repr_directives() {
    let program = parse(
        r#"
@target(.macos) {
    extern "C" {
        fn getpid() -> Int32
    }
}

@repr(.c)
struct Point {
    x: Float64
    y: Float64
}
"#,
    );

    assert_eq!(program.items.len(), 2);
}

#[test]
fn parses_struct_interface_conformance_and_methods() {
    let program = parse(
        r#"
interface Writer {
    fn write(&mut self, bytes: Slice<Byte>) -> Result<Int, Error>
}

struct File: Writer {
    fd: Int

    fn write(&mut self, bytes: Slice<Byte>) -> Result<Int, Error> {
        self.write_raw(bytes)
    }
}
"#,
    );

    assert_eq!(program.items.len(), 2);
}

#[test]
fn rejects_legacy_impl_blocks() {
    let message = parse_err(
        r#"
impl Writer for File {
    fn write(&mut self) {}
}
"#,
    );

    assert!(message.contains("expected item"));
}

#[test]
fn rejects_legacy_struct_as_conformance() {
    let message = parse_err(
        r#"
struct File as Writer {
}
"#,
    );

    assert!(message.contains("expected {"));
}

#[test]
fn rejects_impl_interface_parameter_shorthand() {
    let message = parse_err(
        r#"
fn print(value: impl Display) {
}
"#,
    );

    assert!(message.contains("expected type"));
}

#[test]
fn parses_std_io_writer_as_borrowed_text() {
    let program = parse_file("std/io.mo");
    let writer = program
        .items
        .iter()
        .find_map(|item| match item {
            Item::Interface(interface) if interface.name == "Writer" => Some(interface),
            _ => None,
        })
        .expect("Writer interface");
    let write = writer
        .methods
        .iter()
        .find(|method| method.name == "write")
        .expect("write method");

    assert!(matches!(
        write.params.first().and_then(|param| param.ty_expr.as_ref()),
        Some(TypeExpr::Ref {
            mutable: false,
            inner,
        }) if matches!(inner.as_ref(), TypeExpr::Path(path) if path == &vec!["Str".to_string()])
    ));
}

#[test]
fn parses_index_expression() {
    let program = parse(
        r#"
fn main() -> Int {
    let value = bytes[1 + 1]
    return value
}
"#,
    );

    let function = program
        .items
        .iter()
        .find_map(|item| match item {
            Item::Function(function) if function.name == "main" => Some(function),
            _ => None,
        })
        .expect("main function");
    let stmt = function
        .body
        .as_ref()
        .expect("function body")
        .statements
        .first()
        .expect("let statement");

    assert!(matches!(
        &stmt.data,
        StmtData::Let(let_stmt)
            if matches!(let_stmt.value.as_ref(), Some(Expr::Index(index))
                if matches!(index.target.as_ref(), Expr::Ident(name) if name == "bytes")
                    && matches!(index.index.as_ref(), Expr::Binary(_)))
    ));
}

#[test]
fn parses_all_reference_examples() {
    let paths = [
        "examples/reference/core.mo",
        "examples/reference/types.mo",
        "examples/reference/methods_interfaces.mo",
        "examples/reference/memory_errors.mo",
        "examples/reference/closures_async_threads.mo",
        "examples/reference/platform.mo",
        "examples/reference/web_server.mo",
    ];

    for path in paths {
        let program = parse_file(path);
        assert!(!program.items.is_empty(), "{path} should contain items");
    }
}

#[test]
fn parses_testing_feature() {
    let program = parse_file("examples/reference/core.mo");
    let tests: Vec<_> = program
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Test(test) => Some(test.name.as_str()),
            _ => None,
        })
        .collect();

    assert_eq!(tests, vec!["core bindings and functions parse"]);
}

#[test]
fn parses_type_const_and_static_items() {
    let program = parse_file("examples/reference/core.mo");

    assert!(program
        .items
        .iter()
        .any(|item| matches!(item, Item::TypeAlias(alias) if alias.name == "Handler")));
    assert!(program
        .items
        .iter()
        .any(|item| matches!(item, Item::Const(item) if item.name == "DEFAULT_PORT")));
    assert!(program
        .items
        .iter()
        .any(|item| matches!(item, Item::Static(item) if item.name == "STARTED")));
}

#[test]
fn parses_platform_directive_items() {
    let program = parse_file("examples/reference/platform.mo");

    let directives = program
        .items
        .iter()
        .filter(|item| matches!(item, Item::Directive(_)))
        .count();

    assert_eq!(directives, 4);
}
