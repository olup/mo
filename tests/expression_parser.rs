use mo::ast::{Expr, Item, StmtData};
use mo::{Lexer, Parser};

fn parse(source: &str) -> mo::ast::Program {
    let tokens = Lexer::new(source).tokenize().expect("lex");
    Parser::new(tokens).parse_program().expect("parse")
}

fn function_body(source: &str) -> mo::ast::Block {
    parse(source)
        .items
        .into_iter()
        .find_map(|item| match item {
            Item::Function(function) => function.body,
            _ => None,
        })
        .expect("function body")
}

#[test]
fn parses_let_return_binary_and_call_expressions() {
    let body = function_body(
        r#"
fn sample() -> Int {
    let mut total = add(1, 2)
    return total + 3
}
"#,
    );

    let StmtData::Let(let_stmt) = &body.statements[0].data else {
        panic!("expected let");
    };
    assert!(let_stmt.mutable);
    assert_eq!(let_stmt.name, "total");
    assert!(matches!(let_stmt.value, Some(Expr::Call(_))));

    let StmtData::Return(Some(expr)) = &body.statements[1].data else {
        panic!("expected return");
    };
    assert!(matches!(expr, Expr::Binary(_)));
}

#[test]
fn parses_method_chains_await_try_and_generics() {
    let body = function_body(
        r#"
async fn main() -> Result<(), Error> {
    Server.new()
        .workers(thread.cpu_count())
        .get("/users/:id", get_user)
        .listen("127.0.0.1:3000")
        .await
}
"#,
    );

    let StmtData::Expr(expr) = &body.statements[0].data else {
        panic!("expected expr");
    };
    assert!(matches!(expr, Expr::Await(_)));
}

#[test]
fn parses_struct_literals_and_match_expressions() {
    let body = function_body(
        r#"
fn handle(msg: Message) -> Int {
    let user = User { id: 1, name: "Ada" }
    match msg {
        Quit => 0
        Move { x, y } => x + y
    }
}
"#,
    );

    let StmtData::Let(let_stmt) = &body.statements[0].data else {
        panic!("expected let");
    };
    assert!(matches!(let_stmt.value, Some(Expr::Struct(_))));

    let StmtData::Match(expr) = &body.statements[1].data else {
        panic!("expected match expression statement");
    };
    assert_eq!(expr.arms.len(), 2);
}

#[test]
fn parses_closures_and_async_closures() {
    let body = function_body(
        r#"
fn closures() {
    let label = fn(id: Int) -> String {
        id.to_string()
    }
    let handler = async fn(req: Request) -> Result<Response, Error> {
        Response.text("ok")
    }
    thread.spawn(move fn() {
        print("hello")
    })
}
"#,
    );

    let StmtData::Let(label) = &body.statements[0].data else {
        panic!("expected closure let");
    };
    assert!(matches!(label.value, Some(Expr::Closure(_))));

    let StmtData::Let(handler) = &body.statements[1].data else {
        panic!("expected async closure let");
    };
    assert!(matches!(&handler.value, Some(Expr::Closure(closure)) if closure.is_async));

    let StmtData::Expr(expr) = &body.statements[2].data else {
        panic!("expected spawn expression");
    };
    assert!(matches!(expr, Expr::Call(_)));
}

#[test]
fn parses_generic_call_type_arguments() {
    let body = function_body(
        r#"
fn main() {
    channel.send<String>(ch, message)
}
"#,
    );

    let StmtData::Expr(Expr::Call(call)) = &body.statements[0].data else {
        panic!("expected generic call expression");
    };
    assert_eq!(call.type_args.as_deref(), Some("String"));
}

#[test]
fn parses_mut_arguments_and_object_literals() {
    let body = function_body(
        r#"
fn sample() {
    rename(mut user, "Grace")
    json.object({ "time": time.now() })
}
"#,
    );

    let StmtData::Expr(Expr::Call(call)) = &body.statements[0].data else {
        panic!("expected rename call");
    };
    assert!(matches!(call.args.first(), Some(Expr::Mut(_))));

    let StmtData::Expr(Expr::Call(call)) = &body.statements[1].data else {
        panic!("expected json.object call");
    };
    assert!(matches!(call.args.first(), Some(Expr::Object(object)) if object.fields.len() == 1));
}
