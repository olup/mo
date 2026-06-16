use mo::borrow::check_borrows;
use mo::dropck::{check_drops, DropReport};
use mo::hir::lower_program;
use mo::ownership::check_ownership;
use mo::resolve::resolve_program;
use mo::semantics::{check_program, Target};
use mo::typeck::type_check_program;
use mo::{Lexer, Parser};

fn drop_check(source: &str) -> Result<DropReport, Vec<mo::semantics::Diagnostic>> {
    let tokens = Lexer::new(source).tokenize().expect("lex");
    let program = Parser::new(tokens).parse_program().expect("parse");
    let target = Target::macos_aarch64();
    check_program(&program, &target)?;
    let hir = lower_program(&program, &target)?;
    resolve_program(&hir)?;
    type_check_program(&hir)?;
    check_ownership(&hir)?;
    check_borrows(&hir)?;
    check_drops(&hir)
}

#[test]
fn drops_non_copy_locals_in_reverse_declaration_order() {
    let report = drop_check(
        r#"
fn main() {
    let first = "Ada"
    let second = "Grace"
}
"#,
    )
    .expect("drop check");

    assert_eq!(
        report
            .function_drops
            .get("main")
            .cloned()
            .unwrap_or_default(),
        vec!["second".to_string(), "first".to_string()]
    );
}

#[test]
fn ordinary_owned_structs_still_need_drops() {
    let report = drop_check(
        r#"
struct User {
    name: String
}

extern "C" {
    fn raw_string_concat(a: Str, b: Str) -> String
}

fn main() {
    let user = User { name: raw_string_concat("Ada", "") }
}
"#,
    )
    .expect("drop check");

    assert!(report
        .function_drops
        .get("main")
        .cloned()
        .unwrap_or_default()
        .contains(&"user".to_string()));
}

#[test]
fn if_expression_owned_value_gets_outer_drop() {
    let report = drop_check(
        r#"
extern "C" {
    fn raw_string_concat(a: Str, b: Str) -> String
}

fn main(flag: Bool) {
    let value = if flag {
        raw_string_concat("Ada", "")
    } else {
        raw_string_concat("Grace", "")
    }
}
"#,
    )
    .expect("drop check");

    assert!(report
        .function_drops
        .get("main")
        .cloned()
        .unwrap_or_default()
        .contains(&"value".to_string()));
}

#[test]
fn channel_shared_handles_still_get_wrapper_drops() {
    let report = drop_check(
        r#"
struct Channel {
    raw: Int
}

fn main() {
    let ch = Channel { raw: 0 }
}
"#,
    )
    .expect("drop check");

    assert!(report
        .function_drops
        .get("main")
        .cloned()
        .unwrap_or_default()
        .contains(&"ch".to_string()));
}

#[test]
fn typed_tcp_owners_need_automatic_drops() {
    let report = drop_check(
        r#"
struct TcpListener {
    fd: Int
}

struct TcpStream {
    fd: Int
}

fn main() {
    let listener = TcpListener { fd: 1 }
    let stream = TcpStream { fd: 2 }
}
"#,
    )
    .expect("drop check");

    let drops = report
        .function_drops
        .get("main")
        .cloned()
        .unwrap_or_default();
    assert_eq!(drops, vec!["stream".to_string(), "listener".to_string()]);
}

#[test]
fn explicit_typed_tcp_close_suppresses_automatic_drop() {
    let report = drop_check(
        r#"
struct TcpListener {
    fd: Int
}

struct TcpStream {
    fd: Int
}

fn tcp_listener_close(listener: &TcpListener) -> Int {
    return 0
}

fn tcp_stream_close(stream: &TcpStream) -> Int {
    return 0
}

fn main() {
    let listener = TcpListener { fd: 1 }
    let stream = TcpStream { fd: 2 }
    tcp_stream_close(stream)
    tcp_listener_close(listener)
}
"#,
    )
    .expect("drop check");

    assert!(report
        .function_drops
        .get("main")
        .cloned()
        .unwrap_or_default()
        .is_empty());
}

#[test]
fn copy_locals_do_not_need_drops() {
    let report = drop_check(
        r#"
fn main() {
    let value = 1
}
"#,
    )
    .expect("drop check");

    assert!(report
        .function_drops
        .get("main")
        .cloned()
        .unwrap_or_default()
        .is_empty());
}

#[test]
fn moved_value_is_not_dropped_twice() {
    let report = drop_check(
        r#"
fn main() {
    let first = "Ada"
    let second = first
}
"#,
    )
    .expect("drop check");

    assert_eq!(
        report
            .function_drops
            .get("main")
            .cloned()
            .unwrap_or_default(),
        vec!["second".to_string()]
    );
}

#[test]
fn block_expression_owned_value_gets_outer_drop() {
    let report = drop_check(
        r#"
extern "C" {
    fn raw_string_concat(a: Str, b: Str) -> String
}

fn main() {
    let value = {
        let inner = raw_string_concat("Ada", "")
        inner
    }
}
"#,
    )
    .expect("drop check");

    let drops = report
        .function_drops
        .get("main")
        .cloned()
        .unwrap_or_default();
    assert!(drops.contains(&"value".to_string()));
    assert!(!drops.contains(&"inner".to_string()));
}

#[test]
fn block_expression_move_of_outer_owned_value_suppresses_outer_drop() {
    let report = drop_check(
        r#"
extern "C" {
    fn raw_string_concat(a: Str, b: Str) -> String
}

fn main() {
    let source = raw_string_concat("Ada", "")
    let value = {
        source
    }
}
"#,
    )
    .expect("drop check");

    assert_eq!(
        report
            .function_drops
            .get("main")
            .cloned()
            .unwrap_or_default(),
        vec!["value".to_string()]
    );
}

#[test]
fn specialized_vec_push_consumes_owned_string_payload_drop() {
    let report = drop_check(
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
}
"#,
    )
    .expect("drop check");

    let drops = report
        .function_drops
        .get("main")
        .cloned()
        .unwrap_or_default();
    assert!(drops.contains(&"values".to_string()));
    assert!(!drops.contains(&"name".to_string()));
}

#[test]
fn specialized_map_put_consumes_owned_string_key_and_value_drop() {
    let report = drop_check(
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
}
"#,
    )
    .expect("drop check");

    let drops = report
        .function_drops
        .get("main")
        .cloned()
        .unwrap_or_default();
    assert!(drops.contains(&"values".to_string()));
    assert!(!drops.contains(&"key".to_string()));
    assert!(!drops.contains(&"value".to_string()));
}

#[test]
fn generic_map_destroy_suppresses_automatic_map_drop() {
    let report = drop_check(
        r#"
struct Map<K, V> {
    data: Int
}

fn map__destroy<K, V>(values: &Map<K, V>)

fn main() {
    let values: Map<String, String> = Map { data: 0 }
    map__destroy<String, String>(values)
}
"#,
    )
    .expect("drop check");

    let drops = report
        .function_drops
        .get("main")
        .cloned()
        .unwrap_or_default();
    assert!(!drops.contains(&"values".to_string()));
}

#[test]
fn generic_vec_destroy_suppresses_automatic_vec_drop() {
    let report = drop_check(
        r#"
struct Vec<T> {
    data: Int
}

fn vec__destroy<T>(values: &Vec<T>)

fn main() {
    let values: Vec<String> = Vec { data: 0 }
    vec__destroy<String>(values)
}
"#,
    )
    .expect("drop check");

    let drops = report
        .function_drops
        .get("main")
        .cloned()
        .unwrap_or_default();
    assert!(!drops.contains(&"values".to_string()));
}

#[test]
fn shared_clone_schedules_both_handle_drops() {
    let report = drop_check(
        r#"
struct Shared<T> {
    data: Int
}

fn shared__clone_int(value: &Shared<Int>) -> Shared<Int> {
    return Shared { data: value.data }
}

fn main() {
    let one: Shared<Int> = Shared { data: 42 }
    let two = shared__clone_int(one)
}
"#,
    )
    .expect("drop check");

    assert_eq!(
        report
            .function_drops
            .get("main")
            .cloned()
            .unwrap_or_default(),
        vec!["two".to_string(), "one".to_string()]
    );
}

#[test]
fn early_return_records_initialized_local_drop() {
    let report = drop_check(
        r#"
fn main(flag: Bool) {
    let value = "Ada"
    if flag {
        return
    }
}
"#,
    )
    .expect("drop check");

    assert!(report
        .function_drops
        .get("main")
        .cloned()
        .unwrap_or_default()
        .contains(&"value".to_string()));
}

#[test]
fn move_closure_capture_is_not_dropped_as_original_local() {
    let report = drop_check(
        r#"
fn main() {
    let message = "Ada"
    let handler = move fn() {
        print(message)
    }
}
"#,
    )
    .expect("drop check");

    assert!(!report
        .function_drops
        .get("main")
        .cloned()
        .unwrap_or_default()
        .contains(&"message".to_string()));
}

#[test]
fn nested_move_closure_capture_is_not_dropped_as_original_local() {
    let report = drop_check(
        r#"
fn main() {
    let message = "Ada"
    let handler = move fn() {
        let nested = fn() {
            print(message)
        }
    }
}
"#,
    )
    .expect("drop check");

    assert!(!report
        .function_drops
        .get("main")
        .cloned()
        .unwrap_or_default()
        .contains(&"message".to_string()));
}

#[test]
fn buffer_finish_suppresses_automatic_buffer_drop() {
    let report = drop_check(
        r#"
struct Buffer {
    data: String
}

fn buffer__finish(buffer: &Buffer) -> String {
    return buffer.data
}

extern "C" {
    fn raw_string_concat(a: Str, b: Str) -> String
}

fn main() {
    let buffer = Buffer { data: raw_string_concat("hello", "") }
    let out = buffer__finish(buffer)
}
"#,
    )
    .expect("drop check");

    assert!(!report
        .function_drops
        .get("main")
        .cloned()
        .unwrap_or_default()
        .contains(&"buffer".to_string()));
}

#[test]
fn string_builder_finish_suppresses_automatic_drop() {
    let report = drop_check(
        r#"
struct buffer__StringBuilder {
    data: String
}

fn buffer__string_builder_finish(builder: &buffer__StringBuilder) -> String {
    return builder.data
}

extern "C" {
    fn raw_string_concat(a: Str, b: Str) -> String
}

fn main() {
    let builder = buffer__StringBuilder { data: raw_string_concat("hello", "") }
    let out = buffer__string_builder_finish(builder)
}
"#,
    )
    .expect("drop check");

    assert!(!report
        .function_drops
        .get("main")
        .cloned()
        .unwrap_or_default()
        .contains(&"builder".to_string()));
}

#[test]
fn byte_buffer_finish_suppresses_automatic_drop() {
    let report = drop_check(
        r#"
struct buffer__ByteBuffer {
    data: String
}

fn buffer__byte_buffer_finish(bytes: &buffer__ByteBuffer) -> String {
    return bytes.data
}

extern "C" {
    fn raw_string_concat(a: Str, b: Str) -> String
}

fn main() {
    let bytes = buffer__ByteBuffer { data: raw_string_concat("hello", "") }
    let out = buffer__byte_buffer_finish(bytes)
}
"#,
    )
    .expect("drop check");

    assert!(!report
        .function_drops
        .get("main")
        .cloned()
        .unwrap_or_default()
        .contains(&"bytes".to_string()));
}

#[test]
fn explicit_task_queue_destroy_suppresses_automatic_drop() {
    let report = drop_check(
        r#"
struct TaskQueue4Int {
    raw: Int
}

fn destroy_queue_int(queue: &TaskQueue4Int) -> Int {
    return 0
}

fn main() {
    let queue = TaskQueue4Int { raw: 0 }
    let destroyed = destroy_queue_int(queue)
}
"#,
    )
    .expect("drop check");

    assert!(!report
        .function_drops
        .get("main")
        .cloned()
        .unwrap_or_default()
        .contains(&"queue".to_string()));
}
