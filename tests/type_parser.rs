use mo::ast::{Item, TypeExpr};
use mo::{Lexer, Parser};

fn parse(source: &str) -> mo::ast::Program {
    let tokens = Lexer::new(source).tokenize().expect("lex");
    Parser::new(tokens).parse_program().expect("parse")
}

#[test]
fn parses_reference_and_pointer_types() {
    let program = parse(
        r#"
fn refs(name: &String, user: &mut User, raw: *const Byte, out: *mut Byte) -> &String {
    name
}
"#,
    );

    let function = program
        .items
        .into_iter()
        .find_map(|item| match item {
            Item::Function(function) => Some(function),
            _ => None,
        })
        .expect("function");

    assert!(matches!(
        function.params[0].ty_expr,
        Some(TypeExpr::Ref { mutable: false, .. })
    ));
    assert!(matches!(
        function.params[1].ty_expr,
        Some(TypeExpr::Ref { mutable: true, .. })
    ));
    assert!(matches!(
        function.params[2].ty_expr,
        Some(TypeExpr::RawPtr { mutable: false, .. })
    ));
    assert!(matches!(
        function.params[3].ty_expr,
        Some(TypeExpr::RawPtr { mutable: true, .. })
    ));
    assert!(matches!(
        function.return_type_expr,
        Some(TypeExpr::Ref { mutable: false, .. })
    ));
}

#[test]
fn parses_generic_tuple_and_async_function_types() {
    let program = parse(
        r#"
type Handler = async fn(Request) -> Result<Response, Error>
fn unit() -> Result<(), Error> {}
struct Holder {
    values: Map<String, Vec<Int>>
}
"#,
    );

    let alias = program
        .items
        .iter()
        .find_map(|item| match item {
            Item::TypeAlias(alias) => Some(alias),
            _ => None,
        })
        .expect("alias");
    assert!(matches!(
        alias.value_expr,
        TypeExpr::Fn { is_async: true, .. }
    ));

    let function = program
        .items
        .iter()
        .find_map(|item| match item {
            Item::Function(function) => Some(function),
            _ => None,
        })
        .expect("function");
    assert!(matches!(
        function.return_type_expr,
        Some(TypeExpr::Generic { .. })
    ));

    let holder = program
        .items
        .iter()
        .find_map(|item| match item {
            Item::Struct(item) => Some(item),
            _ => None,
        })
        .expect("struct");
    assert!(matches!(holder.fields[0].ty_expr, TypeExpr::Generic { .. }));
}

#[test]
fn parses_function_type_inside_generic_type_argument() {
    let program = parse(
        r#"
fn main() {
    let ch: channel.Channel<fn() -> ()> = channel.new()
    let job: fn() -> () = channel.recv(ch)
}
"#,
    );

    let function = program
        .items
        .iter()
        .find_map(|item| match item {
            Item::Function(function) => Some(function),
            _ => None,
        })
        .expect("function");
    let stmt = function
        .body
        .as_ref()
        .and_then(|body| body.statements.first())
        .expect("let");
    let mo::ast::StmtData::Let(stmt) = &stmt.data else {
        panic!("expected let statement");
    };
    assert!(matches!(stmt.ty_expr, Some(TypeExpr::Generic { .. })));
}
