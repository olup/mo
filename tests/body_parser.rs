use mo::ast::{Item, StmtKind};
use mo::{Lexer, Parser};

fn parse(source: &str) -> mo::ast::Program {
    let tokens = Lexer::new(source).tokenize().expect("lex");
    Parser::new(tokens).parse_program().expect("parse")
}

fn first_function_body(source: &str) -> mo::ast::Block {
    let program = parse(source);
    let function = program
        .items
        .into_iter()
        .find_map(|item| match item {
            Item::Function(function) => Some(function),
            _ => None,
        })
        .expect("function item");
    function.body.expect("function body")
}

#[test]
fn parses_function_body_statement_kinds() {
    let body = first_function_body(
        r#"
fn sample(items: Slice<Int>) -> Int {
    let mut total = 0
    for item in items {
        total = total + item
    }
    while total < 10 {
        total = total + 1
    }
    return total
}
"#,
    );

    let kinds: Vec<_> = body.statements.iter().map(|stmt| stmt.kind).collect();
    assert_eq!(
        kinds,
        vec![
            StmtKind::Let,
            StmtKind::For,
            StmtKind::While,
            StmtKind::Return
        ]
    );
}

#[test]
fn parses_match_unsafe_and_expression_statements() {
    let body = first_function_body(
        r#"
fn handle(msg: Message) -> Int {
    match msg {
        Quit => 0
        Move { x, y } => x + y
    }
    unsafe {
        raw_touch()
    }
    done()
}
"#,
    );

    let kinds: Vec<_> = body.statements.iter().map(|stmt| stmt.kind).collect();
    assert_eq!(
        kinds,
        vec![StmtKind::Match, StmtKind::Unsafe, StmtKind::Expr]
    );
}

#[test]
fn parses_test_body_as_block() {
    let program = parse(
        r#"
test "body parser" {
    let value = 1
    assert(value == 1)
}
"#,
    );

    let test = program
        .items
        .into_iter()
        .find_map(|item| match item {
            Item::Test(test) => Some(test),
            _ => None,
        })
        .expect("test item");

    let kinds: Vec<_> = test.body.statements.iter().map(|stmt| stmt.kind).collect();
    assert_eq!(kinds, vec![StmtKind::Let, StmtKind::Expr]);
}
