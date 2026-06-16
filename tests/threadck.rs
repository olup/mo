use mo::borrow::check_borrows;
use mo::dropck::check_drops;
use mo::hir::lower_program;
use mo::ownership::check_ownership;
use mo::resolve::resolve_program;
use mo::semantics::{check_program, Target};
use mo::threadck::check_threads;
use mo::typeck::type_check_program;
use mo::{Lexer, Parser};

fn thread_check(
    source: &str,
) -> Result<mo::threadck::ThreadReport, Vec<mo::semantics::Diagnostic>> {
    let tokens = Lexer::new(source).tokenize().expect("lex");
    let program = Parser::new(tokens).parse_program().expect("parse");
    let target = Target::macos_aarch64();
    check_program(&program, &target)?;
    let hir = lower_program(&program, &target)?;
    resolve_program(&hir)?;
    type_check_program(&hir)?;
    check_ownership(&hir)?;
    check_borrows(&hir)?;
    check_drops(&hir)?;
    check_threads(&hir)
}

#[test]
fn move_closure_with_send_capture_can_spawn() {
    thread_check(
        r#"
fn main() {
    let message = "Ada"
    let handle = thread.spawn(move fn() {
        print(message)
    })
}
"#,
    )
    .expect("thread check");
}

#[test]
fn spawn_requires_move_closure() {
    let errors = thread_check(
        r#"
fn main() {
    let message = "Ada"
    let handle = thread.spawn(fn() {
        print(message)
    })
}
"#,
    )
    .expect_err("expected move closure error");

    assert!(errors.iter().any(|error| error
        .message
        .contains("thread.spawn requires a move closure")));
}

#[test]
fn raw_pointer_capture_is_not_send() {
    let errors = thread_check(
        r#"
fn main(ptr: *mut Byte) {
    let handle = thread.spawn(move fn() {
        print(ptr)
    })
}
"#,
    )
    .expect_err("expected non-send capture");

    assert!(errors
        .iter()
        .any(|error| error.message.contains("capture `ptr` is not Send")));
}

#[test]
fn borrowed_reference_parameter_capture_cannot_spawn() {
    let errors = thread_check(
        r#"
fn main(name: &String) {
    let handle = thread.spawn(move fn() {
        print(name)
    })
}
"#,
    )
    .expect_err("expected borrowed reference capture error");

    assert!(errors.iter().any(|error| error
        .message
        .contains("capture `name` is a borrowed reference and cannot be sent to a thread")));
}

#[test]
fn local_borrow_capture_cannot_spawn() {
    let errors = thread_check(
        r#"
fn main() {
    let name = "Ada"
    let borrowed = &name
    let handle = thread.spawn(move fn() {
        print(borrowed)
    })
}
"#,
    )
    .expect_err("expected local borrow capture error");

    assert!(errors.iter().any(|error| error
        .message
        .contains("capture `borrowed` is a borrowed reference and cannot be sent to a thread")));
}

#[test]
fn nested_closure_capture_inside_thread_is_checked_for_send() {
    let errors = thread_check(
        r#"
fn main(ptr: *mut Byte) {
    let handle = thread.spawn(move fn() {
        let nested = fn() {
            print(ptr)
        }
    })
}
"#,
    )
    .expect_err("expected nested non-send capture error");

    assert!(errors
        .iter()
        .any(|error| error.message.contains("capture `ptr` is not Send")));
}

#[test]
fn borrowed_reference_cannot_be_sent_through_channel() {
    let errors = thread_check(
        r#"
fn main(ch: Channel<Int>, value: &Int) {
    channel.send(ch, value)
}
"#,
    )
    .expect_err("expected borrowed channel send error");

    assert!(errors.iter().any(|error| error
        .message
        .contains("channel send value is a borrowed reference and is not Send")));
}

#[test]
fn raw_pointer_cannot_be_sent_through_channel() {
    let errors = thread_check(
        r#"
fn main(ch: Channel<Int>, ptr: *mut Byte) {
    channel.send(ch, ptr)
}
"#,
    )
    .expect_err("expected raw pointer channel send error");

    assert!(errors
        .iter()
        .any(|error| error.message.contains("channel send value is not Send")));
}
