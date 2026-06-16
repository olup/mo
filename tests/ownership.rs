use mo::hir::lower_program;
use mo::ownership::{check_ownership, OwnershipReport};
use mo::resolve::resolve_program;
use mo::semantics::{check_program, Target};
use mo::typeck::type_check_program;
use mo::{Lexer, Parser};

fn ownership_check(source: &str) -> Result<OwnershipReport, Vec<mo::semantics::Diagnostic>> {
    let tokens = Lexer::new(source).tokenize().expect("lex");
    let program = Parser::new(tokens).parse_program().expect("parse");
    let target = Target::macos_aarch64();
    check_program(&program, &target)?;
    let hir = lower_program(&program, &target)?;
    resolve_program(&hir)?;
    type_check_program(&hir)?;
    check_ownership(&hir)
}

#[test]
fn moved_string_cannot_be_reused() {
    let errors = ownership_check(
        r#"
fn consume(name: String) {
}

extern "C" {
    fn raw_string_concat(a: Str, b: Str) -> String
}

fn main() {
    let name = raw_string_concat("Ada", "")
    consume(name)
    print(name)
}
"#,
    )
    .expect_err("expected use after move");

    assert!(errors
        .iter()
        .any(|error| error.message.contains("use of moved value `name`")));
}

#[test]
fn copy_int_can_be_reused_after_call() {
    ownership_check(
        r#"
fn consume(value: Int) {
}

fn main() {
    let value = 1
    consume(value)
    consume(value)
}
"#,
    )
    .expect("ownership check");
}

#[test]
fn borrowed_value_is_not_moved() {
    ownership_check(
        r#"
struct User {
    name: String
}

fn read(user: &User) {
}

extern "C" {
    fn raw_string_concat(a: Str, b: Str) -> String
}

fn main() {
    let user = User { name: raw_string_concat("Ada", "") }
    read(&user)
    read(&user)
}
"#,
    )
    .expect("ownership check");
}

#[test]
fn moving_field_out_of_owned_struct_is_diagnostic() {
    let errors = ownership_check(
        r#"
struct User {
    name: String
}

fn consume(name: String) {
}

extern "C" {
    fn raw_string_concat(a: Str, b: Str) -> String
}

fn main() {
    let user = User { name: raw_string_concat("Ada", "") }
    consume(user.name)
}
"#,
    )
    .expect_err("expected field move diagnostic");

    assert!(errors.iter().any(|error| error.message.contains(
        "cannot move field `name` out of `user`; move-out from aggregate fields is not supported yet"
    )));
}

#[test]
fn borrowed_field_read_does_not_move_struct() {
    ownership_check(
        r#"
struct User {
    name: String
}

fn len(value: &Str) -> Int {
    return 0
}

extern "C" {
    fn raw_string_concat(a: Str, b: Str) -> String
}

fn main() {
    let user = User { name: raw_string_concat("Ada", "") }
    len(user.name)
    len(user.name)
}
"#,
    )
    .expect("borrowed field reads should not move the struct");
}

#[test]
fn owned_string_can_be_read_through_str_parameter_without_move() {
    ownership_check(
        r#"
fn len(value: &Str) -> Int {
    return 0
}

fn main() {
    let name = "Ada"
    len(name)
    print(name)
}
"#,
    )
    .expect("Str parameter should borrow owned string-compatible values");
}

#[test]
fn string_clone_leaves_original_usable() {
    ownership_check(
        r#"
fn string__clone(value: &Str) -> String {
    return raw_string_concat(value, "")
}

extern "C" {
    fn raw_string_concat(a: Str, b: Str) -> String
}

fn main() {
    let name = "Ada"
    let copy = string__clone(name)
    print(name)
    print(copy)
}
"#,
    )
    .expect("String clone should borrow the source and return a distinct owned value");
}

#[test]
fn function_value_string_parameter_move_prevents_reuse() {
    let err = ownership_check(
        r#"
extern "C" {
    fn raw_string_concat(a: Str, b: Str) -> String
}

fn consume(value: String) {
}

fn apply(callback: fn(String) -> (), value: String) {
    callback(value)
    print(value)
}

fn main() {
    let name = raw_string_concat("Ada", "")
    apply(consume, name)
}
"#,
    )
    .expect_err("function-valued String parameter should consume the argument");

    assert!(err
        .iter()
        .any(|diagnostic| diagnostic.message.contains("use of moved value `value`")));
}

#[test]
fn function_value_borrowed_str_parameter_leaves_string_usable() {
    ownership_check(
        r#"
extern "C" {
    fn raw_string_concat(a: Str, b: Str) -> String
}

fn inspect(value: &Str) {
}

fn apply(callback: fn(&Str) -> (), value: String) {
    callback(value)
    print(value)
}

fn main() {
    let name = raw_string_concat("Ada", "")
    apply(inspect, name)
}
"#,
    )
    .expect("function-valued &Str parameter should borrow the argument");
}

#[test]
fn mutable_borrow_call_does_not_move() {
    ownership_check(
        r#"
struct User {
    name: String
}

fn rename(user: &mut User) {
}

extern "C" {
    fn raw_string_concat(a: Str, b: Str) -> String
}

fn main() {
    let mut user = User { name: raw_string_concat("Ada", "") }
    rename(mut user)
    rename(mut user)
}
"#,
    )
    .expect("ownership check");
}

#[test]
fn assignment_moves_non_copy_value() {
    let errors = ownership_check(
        r#"
fn main() {
    let a = "Ada"
    let b = a
    print(a)
}
"#,
    )
    .expect_err("expected use after move");

    assert!(errors
        .iter()
        .any(|error| error.message.contains("use of moved value `a`")));
}

#[test]
fn move_closure_capture_moves_non_copy_value() {
    let errors = ownership_check(
        r#"
fn main() {
    let message = "Ada"
    let handler = move fn() {
        print(message)
    }
    print(message)
}
"#,
    )
    .expect_err("expected move closure capture to move value");

    assert!(errors
        .iter()
        .any(|error| error.message.contains("use of moved value `message`")));
}

#[test]
fn non_move_closure_capture_does_not_move_value() {
    ownership_check(
        r#"
fn main() {
    let message = "Ada"
    let handler = fn() {
        print(message)
    }
    print(message)
}
"#,
    )
    .expect("non-move closure should borrow/read captures");
}

#[test]
fn block_expression_move_of_string_prevents_reuse() {
    let errors = ownership_check(
        r#"
extern "C" {
    fn raw_string_concat(a: Str, b: Str) -> String
}

fn main() {
    let a = raw_string_concat("Ada", "")
    let b = {
        a
    }
    print(a)
    print(b)
}
"#,
    )
    .expect_err("expected use after move through block expression");

    assert!(errors
        .iter()
        .any(|error| error.message.contains("use of moved value `a`")));
}

#[test]
fn if_expression_move_of_string_prevents_reuse() {
    let errors = ownership_check(
        r#"
extern "C" {
    fn raw_string_concat(a: Str, b: Str) -> String
}

fn main(flag: Bool) {
    let a = raw_string_concat("Ada", "")
    let b = if flag {
        a
    } else {
        raw_string_concat("Grace", "")
    }
    print(a)
    print(b)
}
"#,
    )
    .expect_err("expected use after move through if expression");

    assert!(errors
        .iter()
        .any(|error| error.message.contains("use of moved value `a`")));
}

#[test]
fn if_statement_move_of_string_prevents_reuse() {
    let errors = ownership_check(
        r#"
fn consume(value: String) {
}

extern "C" {
    fn raw_string_concat(a: Str, b: Str) -> String
}

fn main(flag: Bool) {
    let a = raw_string_concat("Ada", "")
    if flag {
        consume(a)
    }
    print(a)
}
"#,
    )
    .expect_err("expected use after move through if statement");

    assert!(errors
        .iter()
        .any(|error| error.message.contains("use of moved value `a`")));
}

#[test]
fn match_expression_move_of_string_prevents_reuse() {
    let errors = ownership_check(
        r#"
enum Choice {
    Use
    Other
}

extern "C" {
    fn raw_string_concat(a: Str, b: Str) -> String
}

fn main(choice: Choice) {
    let a = raw_string_concat("Ada", "")
    let b = match choice {
        Use => a
        Other => raw_string_concat("Grace", "")
    }
    print(a)
    print(b)
}
"#,
    )
    .expect_err("expected use after move through match expression");

    assert!(errors
        .iter()
        .any(|error| error.message.contains("use of moved value `a`")));
}

#[test]
fn match_statement_move_of_string_prevents_reuse() {
    let errors = ownership_check(
        r#"
enum Choice {
    Use
    Other
}

fn consume(value: String) {
}

extern "C" {
    fn raw_string_concat(a: Str, b: Str) -> String
}

fn main(choice: Choice) {
    let a = raw_string_concat("Ada", "")
    match choice {
        Use => consume(a)
        Other => print("other")
    }
    print(a)
}
"#,
    )
    .expect_err("expected use after move through match statement");

    assert!(errors
        .iter()
        .any(|error| error.message.contains("use of moved value `a`")));
}

#[test]
fn while_body_move_of_string_prevents_reuse() {
    let errors = ownership_check(
        r#"
fn consume(value: String) {
}

extern "C" {
    fn raw_string_concat(a: Str, b: Str) -> String
}

fn main(flag: Bool) {
    let a = raw_string_concat("Ada", "")
    while flag {
        consume(a)
    }
    print(a)
}
"#,
    )
    .expect_err("expected use after move through while body");

    assert!(errors
        .iter()
        .any(|error| error.message.contains("use of moved value `a`")));
}

#[test]
fn for_body_move_of_string_prevents_reuse() {
    let errors = ownership_check(
        r#"
fn consume(value: String) {
}

extern "C" {
    fn raw_string_concat(a: Str, b: Str) -> String
}

fn main(items: Int) {
    let a = raw_string_concat("Ada", "")
    for item in items {
        consume(a)
    }
    print(a)
}
"#,
    )
    .expect_err("expected use after move through for body");

    assert!(errors
        .iter()
        .any(|error| error.message.contains("use of moved value `a`")));
}

#[test]
fn unsafe_block_move_of_string_prevents_reuse() {
    let errors = ownership_check(
        r#"
fn consume(value: String) {
}

extern "C" {
    fn raw_string_concat(a: Str, b: Str) -> String
}

fn main() {
    let a = raw_string_concat("Ada", "")
    unsafe {
        consume(a)
    }
    print(a)
}
"#,
    )
    .expect_err("expected use after move through unsafe block");

    assert!(errors
        .iter()
        .any(|error| error.message.contains("use of moved value `a`")));
}

#[test]
fn loop_break_after_move_of_string_prevents_reuse() {
    let errors = ownership_check(
        r#"
fn consume(value: String) {
}

extern "C" {
    fn raw_string_concat(a: Str, b: Str) -> String
}

fn main() {
    let a = raw_string_concat("Ada", "")
    loop {
        consume(a)
        break
    }
    print(a)
}
"#,
    )
    .expect_err("expected use after move before loop break");

    assert!(errors
        .iter()
        .any(|error| error.message.contains("use of moved value `a`")));
}

#[test]
fn loop_break_before_unreachable_move_keeps_value_usable() {
    ownership_check(
        r#"
fn consume(value: String) {
}

extern "C" {
    fn raw_string_concat(a: Str, b: Str) -> String
}

fn main() {
    let a = raw_string_concat("Ada", "")
    loop {
        break
        consume(a)
    }
    print(a)
}
"#,
    )
    .expect("unreachable move after break should not move value");
}

#[test]
fn loop_break_inside_if_after_move_of_string_prevents_reuse() {
    let errors = ownership_check(
        r#"
fn consume(value: String) {
}

extern "C" {
    fn raw_string_concat(a: Str, b: Str) -> String
}

fn main(flag: Bool) {
    let a = raw_string_concat("Ada", "")
    loop {
        if flag {
            consume(a)
            break
        }
        break
    }
    print(a)
}
"#,
    )
    .expect_err("expected use after move before nested loop break");

    assert!(errors
        .iter()
        .any(|error| error.message.contains("use of moved value `a`")));
}

#[test]
fn box_take_moves_box_binding() {
    let errors = ownership_check(
        r#"
import * as box from "std/box"

fn main() {
    let value: box.Box<Int> = box.new(42)
    let out = box.take(value)
    let again = box.get_int(value)
}
"#,
    )
    .expect_err("expected box take to move value");

    assert!(errors
        .iter()
        .any(|error| error.message.contains("use of moved value `value`")));
}

#[test]
fn box_string_take_moves_box_binding() {
    let errors = ownership_check(
        r#"
import * as box from "std/box"

fn main() {
    let value: box.Box<String> = box.new("Ada")
    let out = box.take(value)
    let again = box.take(value)
}
"#,
    )
    .expect_err("expected box string take to move value");

    assert!(errors
        .iter()
        .any(|error| error.message.contains("use of moved value `value`")));
}

#[test]
fn borrowed_box_can_be_read_after_get() {
    ownership_check(
        r#"
import * as box from "std/box"

fn main() {
    let value: box.Box<Int> = box.new(42)
    let one = box.get_int(value)
    let two = box.get_int(value)
}
"#,
    )
    .expect("box get should read without moving");
}

#[test]
fn specialized_vec_push_moves_owned_string_payload() {
    let errors = ownership_check(
        r#"
struct Vec<T> {
    data: Int
}

fn vec__push_string(values: &mut Vec<String>, value: String) -> Int {
    return 0
}

extern "C" {
    fn raw_string_concat(a: Str, b: Str) -> String
}

fn main() {
    let values: Vec<String> = Vec { data: 0 }
    let name = raw_string_concat("Ada", "")
    vec__push_string(values, name)
    print(name)
}
"#,
    )
    .expect_err("expected vec push to move string payload");

    assert!(errors
        .iter()
        .any(|error| error.message.contains("use of moved value `name`")));
}

#[test]
fn specialized_map_put_moves_owned_string_key_and_value() {
    let errors = ownership_check(
        r#"
struct Map<K, V> {
    data: Int
}

fn map__put_string_string(values: &mut Map<String, String>, key: String, value: String) -> Int {
    return 0
}

extern "C" {
    fn raw_string_concat(a: Str, b: Str) -> String
}

fn main() {
    let values: Map<String, String> = Map { data: 0 }
    let key = raw_string_concat("starter", "")
    let value = raw_string_concat("pikachu", "")
    map__put_string_string(values, key, value)
    print(key)
    print(value)
}
"#,
    )
    .expect_err("expected map put to move string key and value");

    assert!(errors
        .iter()
        .any(|error| error.message.contains("use of moved value `key`")));
    assert!(errors
        .iter()
        .any(|error| error.message.contains("use of moved value `value`")));
}

#[test]
fn shared_clone_reads_original_handle() {
    ownership_check(
        r#"
struct Shared<T> {
    data: Int
}

fn shared__clone_int(value: &Shared<Int>) -> Shared<Int> {
    return Shared { data: value.data }
}

fn shared__get_int(value: &Shared<Int>) -> Int {
    return value.data
}

fn main() {
    let one: Shared<Int> = Shared { data: 42 }
    let two = shared__clone_int(one)
    let a = shared__get_int(one)
    let b = shared__get_int(two)
}
"#,
    )
    .expect("shared clone should borrow the source handle");
}

#[test]
fn thread_spawn_move_closure_consumes_non_copy_capture() {
    let errors = ownership_check(
        r#"
fn main() {
    let message = "Ada"
    let handle = thread.spawn(move fn() {
        print(message)
    })
    print(message)
}
"#,
    )
    .expect_err("expected thread move closure capture to move value");

    assert!(errors
        .iter()
        .any(|error| error.message.contains("use of moved value `message`")));
}

#[test]
fn nested_closure_capture_inside_move_closure_moves_outer_value() {
    let errors = ownership_check(
        r#"
fn main() {
    let message = "Ada"
    let handler = move fn() {
        let nested = fn() {
            print(message)
        }
    }
    print(message)
}
"#,
    )
    .expect_err("expected nested move closure capture to move value");

    assert!(errors
        .iter()
        .any(|error| error.message.contains("use of moved value `message`")));
}
