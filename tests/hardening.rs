use std::process::Command;

fn mo_check_source(source_name: &str, source: &str) -> std::process::Output {
    let path = std::env::temp_dir().join(format!("{source_name}_{}.mo", std::process::id()));
    std::fs::write(&path, source).expect("write temp source");
    Command::new(env!("CARGO_BIN_EXE_mo"))
        .args(["check", path.to_str().expect("utf-8 path")])
        .output()
        .expect("run mo check")
}

fn assert_check_fails_with(source_name: &str, source: &str, expected: &str) {
    let output = mo_check_source(source_name, source);
    assert!(
        !output.status.success(),
        "mo check unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(expected),
        "expected stderr to contain `{expected}`\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        stderr
    );
}

#[test]
fn hardening_rejects_non_move_thread_closure_capture() {
    assert_check_fails_with(
        "hardening_non_move_thread_capture",
        r#"
import * as thread from "std/thread"

fn main() {
    let message = "Ada"
    let handle = thread.spawn(fn() {
        print(message)
    })
}
"#,
        "thread.spawn requires a move closure",
    );
}

#[test]
fn hardening_rejects_borrowed_reference_thread_capture() {
    assert_check_fails_with(
        "hardening_borrowed_thread_capture",
        r#"
import * as thread from "std/thread"

fn main() {
    let message = "Ada"
    let borrowed = &message
    let handle = thread.spawn(move fn() {
        print(borrowed)
    })
}
"#,
        "capture `borrowed` is a borrowed reference and cannot be sent to a thread",
    );
}

#[test]
fn hardening_rejects_reusing_moved_thread_capture() {
    assert_check_fails_with(
        "hardening_reuse_moved_thread_capture",
        r#"
import * as thread from "std/thread"

fn main() {
    let message = "Ada"
    let handle = thread.spawn(move fn() {
        print(message)
    })
    print(message)
}
"#,
        "use of moved value `message`",
    );
}

#[test]
fn hardening_rejects_borrowed_reference_channel_send() {
    assert_check_fails_with(
        "hardening_borrowed_channel_send",
        r#"
import * as channel from "std/channel"

fn main(ch: channel.Channel<Int>, value: &Int) {
    channel.send(ch, value)
}
"#,
        "channel send value is a borrowed reference and is not Send",
    );
}

#[test]
fn hardening_rejects_raw_pointer_channel_send() {
    assert_check_fails_with(
        "hardening_raw_pointer_channel_send",
        r#"
import * as channel from "std/channel"

fn main(ch: channel.Channel<Int>, ptr: *mut Byte) {
    channel.send(ch, ptr)
}
"#,
        "channel send value is not Send",
    );
}

#[test]
fn hardening_tdd_documents_method_field_name_collision_gap() {
    assert_check_fails_with(
        "hardening_method_field_name_collision",
        r#"
import * as String from "std/string"

struct Router {
    get: fn(Int) -> Int
}

fn inc(value: Int) -> Int {
    return value + 1
}

fn get(router: &mut Router, path: &String, handler: fn(Int) -> Int) -> Int {
    router.get = handler
    return 1
}

fn main() -> Int {
    let mut router = Router { get: inc }
    return router.get("/x", inc)
}
"#,
        "function value expects 1 argument(s), got 2",
    );
}
