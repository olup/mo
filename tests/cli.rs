use std::process::{Command, ExitStatus};

#[cfg(unix)]
use std::os::unix::process::ExitStatusExt;

fn build_and_run_exit_code(example: &str, stem: &str) -> i32 {
    let output_path = std::env::temp_dir().join(format!("{stem}_{}", std::process::id()));
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            example,
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");

    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&output_path)
        .output()
        .expect("run compiled binary");
    run.status.code().unwrap_or_else(|| {
        panic!(
            "compiled binary terminated without exit code: {}",
            describe_status(run.status)
        )
    })
}

fn describe_status(status: ExitStatus) -> String {
    #[cfg(unix)]
    {
        if let Some(signal) = status.signal() {
            return format!("signal {signal}");
        }
    }
    status.to_string()
}

#[test]
fn mo_test_executes_test_blocks() {
    let source = std::env::temp_dir().join(format!("mo_test_pass_{}.mo", std::process::id()));
    std::fs::write(
        &source,
        r#"
test "zero exits ok" {
    return 0
}
"#,
    )
    .expect("write temp source");

    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args(["test", source.to_str().expect("utf-8 path")])
        .output()
        .expect("run mo test");

    assert!(
        output.status.success(),
        "mo test failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("test zero exits ok: ok"));
    assert!(stdout.contains("1/1 test(s) passed"));
}

#[test]
fn mo_test_does_not_drop_moved_enum_locals() {
    let source = std::env::temp_dir().join(format!("mo_test_moved_enum_{}.mo", std::process::id()));
    std::fs::write(
        &source,
        r#"
enum Result<T, E> {
    Ok(T)
    Err(E)
}

fn unwrap_result_or(value: Result<Int, Int>) -> Int {
    return match value {
        Ok(item) => item
        Err(error) => error
    }
}

test "moved enum locals" {
    let ok: Result<Int, Int> = Ok(42)
    let err: Result<Int, Int> = Err(7)
    assert(unwrap_result_or(ok) == 42)
    assert(unwrap_result_or(err) == 7)
}
"#,
    )
    .expect("write temp source");

    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args(["test", source.to_str().expect("utf-8 path")])
        .output()
        .expect("run mo test");

    assert!(
        output.status.success(),
        "mo test failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("test moved enum locals: ok"));
    assert!(stdout.contains("1/1 test(s) passed"));
}

#[test]
fn mo_test_filter_list_and_json_output() {
    let source = std::env::temp_dir().join(format!("mo_test_filter_{}.mo", std::process::id()));
    std::fs::write(
        &source,
        r#"
test "first smoke" {
    return 0
}

test "second memory" {
    return 0
}
"#,
    )
    .expect("write temp source");

    let list = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "test",
            source.to_str().expect("utf-8 path"),
            "--filter",
            "memory",
            "--list",
            "--json",
        ])
        .output()
        .expect("run mo test list");

    assert!(
        list.status.success(),
        "mo test list failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&list.stdout),
        String::from_utf8_lossy(&list.stderr)
    );
    let stdout = String::from_utf8_lossy(&list.stdout);
    assert!(stdout.contains("\"test\":\"second memory\""));
    assert!(!stdout.contains("first smoke"));

    let run = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "test",
            source.to_str().expect("utf-8 path"),
            "--filter",
            "memory",
            "--json",
        ])
        .output()
        .expect("run mo filtered test");

    assert!(
        run.status.success(),
        "mo filtered test failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    let stdout = String::from_utf8_lossy(&run.stdout);
    assert!(stdout.contains("\"status\":\"ok\""));
    assert!(stdout.contains("\"total\":1"));
}

#[test]
fn mo_test_timeout_fails_hung_test() {
    let source = std::env::temp_dir().join(format!("mo_test_timeout_{}.mo", std::process::id()));
    std::fs::write(
        &source,
        r#"
test "hangs" {
    while true {}
    return 0
}
"#,
    )
    .expect("write temp source");

    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "test",
            source.to_str().expect("utf-8 path"),
            "--timeout",
            "0",
        ])
        .output()
        .expect("run mo test timeout");

    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("timeout"));
}

#[test]
fn mo_check_debug_dump_options() {
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "check",
            "examples/compile/return_42.mo",
            "--emit-hir",
            "--emit-ir",
            "--dump-drops",
            "--dump-ownership",
        ])
        .output()
        .expect("run mo check debug dumps");

    assert!(
        output.status.success(),
        "mo check debug dump failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("HirProgram"));
    assert!(stdout.contains("IrProgram"));
    assert!(stdout.contains("DropReport"));
    assert!(stdout.contains("ownership: checked"));
}

#[test]
fn mo_example_runs_named_compile_example() {
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args(["example", "return_42"])
        .output()
        .expect("run mo example");

    assert_eq!(output.status.code(), Some(42));
}

#[test]
fn mo_test_runs_std_library_tests() {
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args(["test", "std/test/std_test.mo"])
        .output()
        .expect("run mo test");

    assert!(
        output.status.success(),
        "mo test failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("test std string helpers: ok"));
    assert!(stdout.contains("test std byte classification and memory helpers: ok"));
    assert!(stdout.contains("test string equality compares values: ok"));
    assert!(stdout.contains("test bool equality compares values: ok"));
    assert!(stdout.contains("test std int helpers: ok"));
    assert!(stdout.contains("test std option helpers: ok"));
    assert!(stdout.contains("test std result helpers: ok"));
    assert!(stdout.contains("test std buffer helpers: ok"));
    assert!(stdout.contains("test std path helpers: ok"));
    assert!(stdout.contains("9/9 test(s) passed"));
}

#[test]
fn mo_test_runs_toml_library_tests() {
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args(["test", "lib/toml/test/toml_test.mo"])
        .output()
        .expect("run mo test");

    assert!(
        output.status.success(),
        "mo test failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("test toml parses package manifest scalars: ok"));
    assert!(stdout.contains("test toml parses dotted sections and native arrays: ok"));
    assert!(stdout.contains("test toml parses scripts and ignores comments outside strings: ok"));
    assert!(stdout.contains("test toml parses typed arrays: ok"));
    assert!(stdout.contains("4/4 test(s) passed"));
}

#[test]
fn mo_test_discovers_dot_test_mo_files() {
    let dir = std::env::temp_dir().join(format!("mo_dot_test_discovery_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp test dir");
    std::fs::write(
        dir.join("sample.test.mo"),
        r#"
test "dot test file runs" {
    return 0
}
"#,
    )
    .expect("write dot test file");

    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args(["test", dir.to_str().expect("utf-8 path")])
        .output()
        .expect("run mo test");

    assert!(
        output.status.success(),
        "mo test failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("sample.test.mo: test dot test file runs: ok"));
    assert!(stdout.contains("1/1 test(s) passed"));
}

#[test]
fn mo_build_writes_metrics_json() {
    let output_path = std::env::temp_dir().join(format!("mo_metrics_build_{}", std::process::id()));
    let metrics_path = output_path.with_extension("json");
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            "examples/compile/return_42.mo",
            "-o",
            output_path.to_str().expect("utf-8 path"),
            "--metrics-json",
            metrics_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");

    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let metrics = std::fs::read_to_string(&metrics_path).expect("read metrics json");
    assert!(metrics.contains("\"build_wall_seconds\""));
    assert!(metrics.contains("\"object_size_bytes\""));
    assert!(metrics.contains("\"executable_size_bytes\""));
    assert!(metrics.contains("\"run_wall_seconds\": null"));
}

#[test]
fn mo_run_watch_starts_and_waits_for_changes() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "run",
            "examples/compile/return_zero.mo",
            "--watch",
            "--debounce-ms",
            "50",
        ])
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("spawn mo run --watch");

    std::thread::sleep(std::time::Duration::from_millis(800));
    let _ = child.kill();
    let output = child.wait_with_output().expect("wait for mo run --watch");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("watching examples/compile/return_zero.mo"));
    assert!(stderr.contains("running "));
}

#[test]
fn mo_run_memory_report_prints_process_and_runtime_guidance() {
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args(["run", "examples/compile/return_zero.mo", "--memory-report"])
        .output()
        .expect("run mo memory report");

    assert!(
        output.status.success(),
        "mo run --memory-report failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("memory report:"));
    assert!(stderr.contains("runtime counters:"));
    assert!(stderr.contains("process max rss bytes:"));
}

#[test]
fn mo_run_writes_metrics_json() {
    let metrics_path =
        std::env::temp_dir().join(format!("mo_metrics_run_{}.json", std::process::id()));
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "run",
            "examples/compile/return_zero.mo",
            "--metrics-json",
            metrics_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo run");

    assert!(
        output.status.success(),
        "mo run failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let metrics = std::fs::read_to_string(&metrics_path).expect("read metrics json");
    assert!(metrics.contains("\"run_wall_seconds\""));
    assert!(metrics.contains("\"run_exit_code\": 0"));
}

#[test]
fn mo_fmt_check_reports_and_fixes_formatting() {
    let source = std::env::temp_dir().join(format!("mo_fmt_{}.mo", std::process::id()));
    std::fs::write(&source, "fn main() -> Int {\nreturn 42\n}\n")
        .expect("write unformatted source");

    let check = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args(["fmt", source.to_str().expect("utf-8 path"), "--check"])
        .output()
        .expect("run mo fmt --check");
    assert!(!check.status.success());

    let fix = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args(["fmt", source.to_str().expect("utf-8 path")])
        .output()
        .expect("run mo fmt");
    assert!(
        fix.status.success(),
        "mo fmt failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&fix.stdout),
        String::from_utf8_lossy(&fix.stderr)
    );
    let formatted = std::fs::read_to_string(&source).expect("read formatted source");
    assert!(formatted.contains("    return 42\n"));

    let check_after = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args(["fmt", source.to_str().expect("utf-8 path"), "--check"])
        .output()
        .expect("run mo fmt --check after fix");
    assert!(check_after.status.success());
}

#[test]
fn mo_test_runs_json_library_tests() {
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args(["test", "lib/test/json_test.mo"])
        .output()
        .expect("run mo test");

    assert!(
        output.status.success(),
        "mo test failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("test json encodes object fields: ok"));
    assert!(stdout.contains("test json parses fields with fallbacks: ok"));
    assert!(stdout.contains("2/2 test(s) passed"));
}

#[test]
fn mo_test_runs_http_library_tests() {
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args(["test", "std/test/http.test.mo"])
        .output()
        .expect("run mo test");

    assert!(
        output.status.success(),
        "mo test failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("test std http response builders: ok"));
    assert!(stdout.contains("test std http header map stores string headers: ok"));
    assert!(stdout.contains("test std http renders error status responses: ok"));
    assert!(stdout.contains("test std http response stores owned headers: ok"));
    assert!(stdout.contains("4/4 test(s) passed"));
}

#[test]
fn mo_test_runs_map_library_tests() {
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args(["test", "std/test/map.test.mo"])
        .output()
        .expect("run mo test");

    assert!(
        output.status.success(),
        "mo test failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("test std map stores and reads owned string values: ok"));
    assert!(stdout.contains("1/1 test(s) passed"));
}

#[test]
fn mo_test_runs_vec_library_tests() {
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args(["test", "std/test/vec.test.mo"])
        .output()
        .expect("run mo test");

    assert!(
        output.status.success(),
        "mo test failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("test std vec int stores indexed values: ok"));
    assert!(stdout.contains("test std vec string stores owned values: ok"));
    assert!(stdout.contains("test std vec handler stores callback values: ok"));
    assert!(stdout.contains("3/3 test(s) passed"));
}

#[test]
fn mo_test_runs_buffer_library_tests() {
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args(["test", "std/test/buffer.test.mo"])
        .output()
        .expect("run mo test");

    assert!(
        output.status.success(),
        "mo test failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("test std buffer appends strings bytes and ints: ok"));
    assert!(stdout.contains("test std buffer grows past initial capacity: ok"));
    assert!(stdout.contains("test std string builder facade appends and finishes text: ok"));
    assert!(stdout.contains("test std byte buffer facade gets sets and grows bytes: ok"));
    assert!(stdout.contains("4/4 test(s) passed"));
}

#[test]
fn mo_test_runs_slice_library_tests() {
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args(["test", "std/test/slice.test.mo"])
        .output()
        .expect("run mo test");

    assert!(
        output.status.success(),
        "mo test failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("test std byte slice views borrowed string bytes: ok"));
    assert!(stdout.contains("test std byte slice clamps invalid ranges: ok"));
    assert!(stdout.contains("2/2 test(s) passed"));
}

#[test]
fn mo_test_runs_channel_library_tests() {
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args(["test", "std/test/channel.test.mo"])
        .output()
        .expect("run mo test");

    assert!(
        output.status.success(),
        "mo test failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("test std channel int send recv: ok"));
    assert!(stdout.contains("test std channel string copies value: ok"));
    assert!(stdout.contains("2/2 test(s) passed"));
}

#[test]
fn mo_test_runs_remaining_std_library_file_tests() {
    let test_files = [
        "std/test/async.test.mo",
        "std/test/atomic.test.mo",
        "std/test/bytes.test.mo",
        "std/test/fs.test.mo",
        "std/test/int.test.mo",
        "std/test/io.test.mo",
        "std/test/net.test.mo",
        "std/test/path.test.mo",
        "std/test/process.test.mo",
        "std/test/shared.test.mo",
        "std/test/sse.test.mo",
        "std/test/string.test.mo",
        "std/test/sync.test.mo",
        "std/test/task.test.mo",
        "std/test/thread.test.mo",
    ];

    for test_file in test_files {
        let output = Command::new(env!("CARGO_BIN_EXE_mo"))
            .args(["test", test_file])
            .output()
            .expect("run mo test");

        assert!(
            output.status.success(),
            "mo test failed for {test_file}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("test(s) passed"), "{test_file}:\n{stdout}");
    }
}

#[test]
fn mo_test_runs_concurrency_hardening_tests() {
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args(["test", "hardening/test/concurrency_test.mo"])
        .output()
        .expect("run mo test");

    assert!(
        output.status.success(),
        "mo test failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("test mutex protects raw shared counter across four threads: ok"));
    assert!(stdout.contains("test function channel transfers named and closure callbacks: ok"));
    assert!(stdout.contains("test method registration mutates function valued handler field: ok"));
    assert!(stdout.contains("10/10 test(s) passed"));
}

#[test]
fn mo_test_runs_tdd_hardening_regressions() {
    let atomic = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args(["test", "hardening/tdd/atomic_clone_test.mo"])
        .output()
        .expect("run mo test");

    assert!(
        atomic.status.success(),
        "mo test failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&atomic.stdout),
        String::from_utf8_lossy(&atomic.stderr)
    );
    let atomic_stdout = String::from_utf8_lossy(&atomic.stdout);
    assert!(atomic_stdout
        .contains("test atomic clones coordinate concurrent increments without crashing: ok"));
    assert!(atomic_stdout.contains("1/1 test(s) passed"));

    let strings = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args(["test", "hardening/tdd/string_channel_test.mo"])
        .output()
        .expect("run mo test");

    assert!(
        strings.status.success(),
        "mo test failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&strings.stdout),
        String::from_utf8_lossy(&strings.stderr)
    );
    let strings_stdout = String::from_utf8_lossy(&strings.stdout);
    assert!(strings_stdout
        .contains("test string channel copies borrowed string across thread without crashing: ok"));
    assert!(strings_stdout.contains("1/1 test(s) passed"));
}

#[test]
fn mo_test_reports_failing_exit_code() {
    let source = std::env::temp_dir().join(format!("mo_test_fail_{}.mo", std::process::id()));
    std::fs::write(
        &source,
        r#"
test "nonzero fails" {
    return 7
}
"#,
    )
    .expect("write temp source");

    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args(["test", source.to_str().expect("utf-8 path")])
        .output()
        .expect("run mo test");

    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("test nonzero fails: failed with exit code 7"));
    assert!(stdout.contains("0/1 test(s) passed"));
}

#[test]
fn mo_test_assertion_passes_when_condition_is_true() {
    let source =
        std::env::temp_dir().join(format!("mo_test_assert_pass_{}.mo", std::process::id()));
    std::fs::write(
        &source,
        r#"
test "assert true" {
    assert(2 < 3)
}
"#,
    )
    .expect("write temp source");

    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args(["test", source.to_str().expect("utf-8 path")])
        .output()
        .expect("run mo test");

    assert!(
        output.status.success(),
        "mo test failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("test assert true: ok"));
}

#[test]
fn mo_test_assertion_fails_when_condition_is_false() {
    let source =
        std::env::temp_dir().join(format!("mo_test_assert_fail_{}.mo", std::process::id()));
    std::fs::write(
        &source,
        r#"
test "assert false" {
    assert(3 < 2)
}
"#,
    )
    .expect("write temp source");

    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args(["test", source.to_str().expect("utf-8 path")])
        .output()
        .expect("run mo test");

    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("test assert false: failed with exit code 1"));
    assert!(stdout.contains("assertion failed"));
    assert!(stdout.contains("0/1 test(s) passed"));
}

#[test]
fn mo_check_reports_semantic_errors() {
    let source = std::env::temp_dir().join("mo_duplicate_symbol.mo");
    std::fs::write(
        &source,
        r#"
fn same() {}
fn same() {}
"#,
    )
    .expect("write temp source");

    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args(["check", source.to_str().expect("utf-8 path")])
        .output()
        .expect("run mo check");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains(&format!("{}:3:1: semantic error", source.display())));
    assert!(stderr.contains("[MO1001]"));
    assert!(stderr.contains("duplicate top-level symbol `same`"));
}

#[test]
fn mo_check_reports_resolve_errors() {
    let source = std::env::temp_dir().join("mo_unknown_value.mo");
    std::fs::write(
        &source,
        r#"
fn main() -> Int {
    return missing
}
"#,
    )
    .expect("write temp source");

    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args(["check", source.to_str().expect("utf-8 path")])
        .output()
        .expect("run mo check");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("unknown value `missing`"));
}

#[test]
fn mo_check_reports_type_errors() {
    let source = std::env::temp_dir().join("mo_type_error.mo");
    std::fs::write(
        &source,
        r#"
fn bad() -> Int {
    return "no"
}
"#,
    )
    .expect("write temp source");

    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args(["check", source.to_str().expect("utf-8 path")])
        .output()
        .expect("run mo check");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("return type mismatch"));
}

#[test]
fn mo_check_reports_lex_errors_with_line_and_column() {
    let source = std::env::temp_dir().join("mo_lex_location.mo");
    std::fs::write(
        &source,
        r#"
fn main() -> Int {
    return $
}
"#,
    )
    .expect("write temp source");

    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args(["check", source.to_str().expect("utf-8 path")])
        .output()
        .expect("run mo check");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains(&format!("{}:3:12: lex error", source.display())));
    assert!(stderr.contains("[MO0001]"));
}

#[test]
fn mo_check_reports_parse_errors_with_line_and_column() {
    let source = std::env::temp_dir().join("mo_parse_location.mo");
    std::fs::write(
        &source,
        r#"
fn main( -> Int {
    return 0
}
"#,
    )
    .expect("write temp source");

    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args(["check", source.to_str().expect("utf-8 path")])
        .output()
        .expect("run mo check");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains(&format!("{}:2:", source.display())));
    assert!(stderr.contains("parse error"));
    assert!(stderr.contains("[MO0002]"));
}

#[test]
fn mo_test_reports_parse_errors_with_line_and_column() {
    let source = std::env::temp_dir().join("mo_test_parse_location.mo");
    std::fs::write(
        &source,
        r#"
test "bad syntax" {
    assert(
}
"#,
    )
    .expect("write temp source");

    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args(["test", source.to_str().expect("utf-8 path")])
        .output()
        .expect("run mo test");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains(&format!("{}:4:", source.display())));
    assert!(stderr.contains("parse error"));
    assert!(stderr.contains("[MO0002]"));
}

#[test]
fn mo_build_compiles_and_runs_return_zero() {
    let output_path = std::env::temp_dir().join(format!("mo_return_zero_{}", std::process::id()));
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            "examples/compile/return_zero.mo",
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");

    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !String::from_utf8_lossy(&output.stderr).contains("no platform load command"),
        "object should include Mach-O platform metadata"
    );

    let run = Command::new(&output_path)
        .output()
        .expect("run compiled binary");
    assert!(run.status.success(), "compiled binary should exit 0");
}

#[test]
fn mo_build_compiles_and_runs_return_42() {
    let output_path = std::env::temp_dir().join(format!("mo_return_42_{}", std::process::id()));
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            "examples/compile/return_42.mo",
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");

    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !String::from_utf8_lossy(&output.stderr).contains("no platform load command"),
        "object should include Mach-O platform metadata"
    );

    let run = Command::new(&output_path)
        .output()
        .expect("run compiled binary");
    assert_eq!(run.status.code(), Some(42));
}

#[test]
fn mo_build_compiles_and_runs_local_integer_return() {
    let output_path = std::env::temp_dir().join(format!("mo_local_return_{}", std::process::id()));
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            "examples/compile/local_return.mo",
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");

    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&output_path)
        .output()
        .expect("run compiled binary");
    assert_eq!(run.status.code(), Some(17));
}

#[test]
fn mo_build_compiles_and_runs_integer_arithmetic() {
    let output_path = std::env::temp_dir().join(format!("mo_arithmetic_{}", std::process::id()));
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            "examples/compile/arithmetic.mo",
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");

    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&output_path)
        .output()
        .expect("run compiled binary");
    assert_eq!(run.status.code(), Some(42));
}

#[test]
fn mo_build_compiles_and_runs_direct_function_call() {
    let output_path = std::env::temp_dir().join(format!("mo_function_call_{}", std::process::id()));
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            "examples/compile/function_call.mo",
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");

    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&output_path)
        .output()
        .expect("run compiled binary");
    assert_eq!(run.status.code(), Some(42));
}

#[test]
fn mo_build_compiles_and_runs_named_function_callback() {
    let source = std::env::temp_dir().join(format!("mo_callback_{}.mo", std::process::id()));
    std::fs::write(
        &source,
        r#"
fn inc(value: Int) -> Int {
    return value + 1
}

fn apply(callback: fn(Int) -> Int, value: Int) -> Int {
    return callback(value)
}

fn main() -> Int {
    return apply(inc, 41)
}
"#,
    )
    .expect("write temp source");

    let output_path = std::env::temp_dir().join(format!("mo_callback_{}", std::process::id()));
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            source.to_str().expect("utf-8 path"),
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");

    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&output_path)
        .output()
        .expect("run compiled binary");
    assert_eq!(run.status.code(), Some(42));
}

#[test]
fn mo_build_compiles_and_runs_non_capturing_closure_callback() {
    let source =
        std::env::temp_dir().join(format!("mo_closure_callback_{}.mo", std::process::id()));
    std::fs::write(
        &source,
        r#"
fn apply(callback: fn(Int) -> Int, value: Int) -> Int {
    return callback(value)
}

fn main() -> Int {
    let inc = fn(value: Int) -> Int {
        return value + 1
    }
    return apply(inc, 41)
}
"#,
    )
    .expect("write temp source");

    let output_path =
        std::env::temp_dir().join(format!("mo_closure_callback_{}", std::process::id()));
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            source.to_str().expect("utf-8 path"),
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");

    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&output_path)
        .output()
        .expect("run compiled binary");
    assert_eq!(run.status.code(), Some(42));
}

#[test]
fn mo_build_compiles_and_runs_thread_spawn_join_non_capturing_closure() {
    let source = std::env::temp_dir().join(format!("mo_thread_spawn_{}.mo", std::process::id()));
    std::fs::write(
        &source,
        r#"
import * as thread from "std/thread"

fn main() -> Int {
    let handle = thread.spawn(move fn() {
        print("thread")
    })
    let joined = thread.join(handle)
    if joined == 0 {
        return 42
    }
    return 1
}
"#,
    )
    .expect("write temp source");

    let output_path = std::env::temp_dir().join(format!("mo_thread_spawn_{}", std::process::id()));
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            source.to_str().expect("utf-8 path"),
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");

    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&output_path)
        .output()
        .expect("run compiled binary");
    assert_eq!(run.status.code(), Some(42));
}

#[test]
fn mo_build_compiles_and_runs_thread_spawn_join_named_function() {
    let source =
        std::env::temp_dir().join(format!("mo_thread_spawn_named_{}.mo", std::process::id()));
    std::fs::write(
        &source,
        r#"
import * as thread from "std/thread"

fn worker() {
    print("worker")
}

fn main() -> Int {
    let handle = thread.spawn(worker)
    let joined = thread.join(handle)
    if joined == 0 {
        return 42
    }
    return 1
}
"#,
    )
    .expect("write temp source");

    let output_path =
        std::env::temp_dir().join(format!("mo_thread_spawn_named_{}", std::process::id()));
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            source.to_str().expect("utf-8 path"),
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");

    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&output_path)
        .output()
        .expect("run compiled binary");
    assert_eq!(run.status.code(), Some(42));
}

#[test]
fn mo_build_compiles_and_runs_thread_spawn_join_captured_string_closure() {
    let source =
        std::env::temp_dir().join(format!("mo_thread_spawn_capture_{}.mo", std::process::id()));
    std::fs::write(
        &source,
        r#"
import * as thread from "std/thread"

fn main() -> Int {
    let message = "captured"
    let handle = thread.spawn(move fn() {
        print(message)
    })
    let joined = thread.join(handle)
    if joined == 0 {
        return 42
    }
    return 1
}
"#,
    )
    .expect("write temp source");

    let output_path =
        std::env::temp_dir().join(format!("mo_thread_spawn_capture_{}", std::process::id()));
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            source.to_str().expect("utf-8 path"),
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");

    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&output_path)
        .output()
        .expect("run compiled binary");
    assert_eq!(run.status.code(), Some(42));
}

#[test]
fn mo_build_compiles_and_runs_thread_spawn_join_multiple_captures() {
    let source = std::env::temp_dir().join(format!(
        "mo_thread_spawn_multi_capture_{}.mo",
        std::process::id()
    ));
    std::fs::write(
        &source,
        r#"
import * as thread from "std/thread"

fn main() -> Int {
    let message = "multi"
    let value = 7
    let handle = thread.spawn(move fn() {
        print(message)
        print(value)
    })
    let joined = thread.join(handle)
    if joined == 0 {
        return 42
    }
    return 1
}
"#,
    )
    .expect("write temp source");

    let output_path = std::env::temp_dir().join(format!(
        "mo_thread_spawn_multi_capture_{}",
        std::process::id()
    ));
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            source.to_str().expect("utf-8 path"),
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");

    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&output_path)
        .output()
        .expect("run compiled binary");
    assert_eq!(run.status.code(), Some(42));
}

#[test]
fn mo_build_compiles_and_runs_thread_spawn_join_captured_bool_closure() {
    let source = std::env::temp_dir().join(format!(
        "mo_thread_spawn_bool_capture_{}.mo",
        std::process::id()
    ));
    std::fs::write(
        &source,
        r#"
import * as thread from "std/thread"

fn main() -> Int {
    let enabled = true
    let handle = thread.spawn(move fn() {
        if enabled {
            print("enabled")
        }
    })
    let joined = thread.join(handle)
    if joined == 0 {
        return 42
    }
    return 1
}
"#,
    )
    .expect("write temp source");

    let output_path = std::env::temp_dir().join(format!(
        "mo_thread_spawn_bool_capture_{}",
        std::process::id()
    ));
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            source.to_str().expect("utf-8 path"),
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");

    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&output_path)
        .output()
        .expect("run compiled binary");
    assert_eq!(run.status.code(), Some(42));
}

#[test]
fn mo_build_compiles_and_runs_thread_spawn_join_captured_struct_closure() {
    let source = std::env::temp_dir().join(format!(
        "mo_thread_spawn_struct_capture_{}.mo",
        std::process::id()
    ));
    std::fs::write(
        &source,
        r#"
import * as thread from "std/thread"
import * as String from "std/string"

struct User {
    name: String
    id: Int
}

fn main() -> Int {
    let user = User { name: String.from("Ada"), id: 41 }
    let handle = thread.spawn(move fn() {
        print(user.name)
    })
    let joined = thread.join(handle)
    if joined == 0 {
        return 42
    }
    return 1
}
"#,
    )
    .expect("write temp source");

    let output_path = std::env::temp_dir().join(format!(
        "mo_thread_spawn_struct_capture_{}",
        std::process::id()
    ));
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            source.to_str().expect("utf-8 path"),
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");

    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&output_path)
        .output()
        .expect("run compiled binary");
    assert_eq!(run.status.code(), Some(42));
}

#[test]
fn mo_build_compiles_and_runs_thread_spawn_join_captured_struct_early_return() {
    let source = std::env::temp_dir().join(format!(
        "mo_thread_spawn_struct_early_return_{}.mo",
        std::process::id()
    ));
    std::fs::write(
        &source,
        r#"
import * as thread from "std/thread"
import * as String from "std/string"

struct User {
    name: String
    id: Int
}

fn main() -> Int {
    let user = User { name: String.from("Ada"), id: 41 }
    let handle = thread.spawn(move fn() {
        if user.id == 41 {
            return
        }
        print(user.name)
    })
    let joined = thread.join(handle)
    if joined == 0 {
        return 42
    }
    return 1
}
"#,
    )
    .expect("write temp source");

    let output_path = std::env::temp_dir().join(format!(
        "mo_thread_spawn_struct_early_return_{}",
        std::process::id()
    ));
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            source.to_str().expect("utf-8 path"),
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");

    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&output_path)
        .output()
        .expect("run compiled binary");
    assert_eq!(run.status.code(), Some(42));
    assert!(
        String::from_utf8_lossy(&run.stdout).trim().is_empty(),
        "early return should skip print, stdout was: {}",
        String::from_utf8_lossy(&run.stdout)
    );
}

#[test]
fn mo_build_compiles_and_runs_thread_spawn_join_captured_enum_closure() {
    let source = std::env::temp_dir().join(format!(
        "mo_thread_spawn_enum_capture_{}.mo",
        std::process::id()
    ));
    std::fs::write(
        &source,
        r#"
import * as thread from "std/thread"
import * as String from "std/string"

struct User {
    name: String
    id: Int
}

enum Lookup {
    Found(User)
    Missing
}

fn main() -> Int {
    let user = User { name: String.from("Ada"), id: 41 }
    let lookup: Lookup = Found(user)
    let handle = thread.spawn(move fn() {
        let value: Int = match lookup {
            Found(found) => found.id
            Missing => 0
        }
        print(value)
    })
    let joined = thread.join(handle)
    if joined == 0 {
        return 42
    }
    return 1
}
"#,
    )
    .expect("write temp source");

    let output_path = std::env::temp_dir().join(format!(
        "mo_thread_spawn_enum_capture_{}",
        std::process::id()
    ));
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            source.to_str().expect("utf-8 path"),
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");

    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&output_path)
        .output()
        .expect("run compiled binary");
    assert_eq!(run.status.code(), Some(42));
}

#[test]
fn mo_build_compiles_and_runs_print_hello() {
    let output_path = std::env::temp_dir().join(format!("mo_print_hello_{}", std::process::id()));
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            "examples/compile/print_hello.mo",
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");

    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&output_path)
        .output()
        .expect("run compiled binary");
    assert_eq!(run.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&run.stdout), "hello\n");
}

#[test]
fn mo_build_compiles_and_runs_if_return() {
    let output_path = std::env::temp_dir().join(format!("mo_if_return_{}", std::process::id()));
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            "examples/compile/if_return.mo",
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");

    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&output_path)
        .output()
        .expect("run compiled binary");
    assert_eq!(run.status.code(), Some(42));
}

#[test]
fn mo_build_compiles_and_runs_if_false_return() {
    let output_path =
        std::env::temp_dir().join(format!("mo_if_false_return_{}", std::process::id()));
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            "examples/compile/if_false_return.mo",
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");

    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&output_path)
        .output()
        .expect("run compiled binary");
    assert_eq!(run.status.code(), Some(42));
}

#[test]
fn mo_build_compiles_and_runs_if_expression_value() {
    assert_eq!(
        build_and_run_exit_code(
            "examples/compile/if_expression_value.mo",
            "mo_if_expression_value"
        ),
        42
    );
}

#[test]
fn mo_build_compiles_and_runs_if_expr_owned_string_drop_memory() {
    assert_eq!(
        build_and_run_exit_code(
            "examples/compile/if_expr_owned_string_drop_memory.mo",
            "mo_if_expr_owned_string_drop_memory"
        ),
        42
    );
}

#[test]
fn mo_build_compiles_and_runs_string_block_expression_drop_memory() {
    assert_eq!(
        build_and_run_exit_code(
            "examples/compile/string_block_expression_drop_memory.mo",
            "mo_string_block_expression_drop_memory"
        ),
        42
    );
}

#[test]
fn mo_build_compiles_and_runs_struct_return_owned_read_drop_memory() {
    assert_eq!(
        build_and_run_exit_code(
            "examples/compile/struct_return_owned_read_drop_memory.mo",
            "mo_struct_return_owned_read_drop_memory"
        ),
        42
    );
}

#[test]
fn mo_build_compiles_and_runs_function_field_return_drop_memory() {
    assert_eq!(
        build_and_run_exit_code(
            "examples/compile/function_field_return_drop_memory.mo",
            "mo_function_field_return_drop_memory"
        ),
        42
    );
}

#[test]
fn mo_build_compiles_and_runs_closure_string_return_drop_memory() {
    assert_eq!(
        build_and_run_exit_code(
            "examples/compile/closure_string_return_drop_memory.mo",
            "mo_closure_string_return_drop_memory"
        ),
        0
    );
}

#[test]
fn mo_run_builds_and_executes_print_hello() {
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args(["run", "examples/compile/print_hello.mo"])
        .output()
        .expect("run mo run");

    assert!(
        output.status.success(),
        "mo run failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "hello\n");
}

#[test]
fn mo_run_preserves_program_exit_code() {
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args(["run", "examples/compile/return_42.mo"])
        .output()
        .expect("run mo run");

    assert_eq!(output.status.code(), Some(42));
}

#[test]
fn mo_build_compiles_and_runs_while_sum() {
    let output_path = std::env::temp_dir().join(format!("mo_while_sum_{}", std::process::id()));
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            "examples/compile/while_sum.mo",
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");

    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&output_path)
        .output()
        .expect("run compiled binary");
    assert_eq!(run.status.code(), Some(45));
}

#[test]
fn mo_build_compiles_and_runs_bool_local() {
    let output_path = std::env::temp_dir().join(format!("mo_bool_local_{}", std::process::id()));
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            "examples/compile/bool_local.mo",
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");

    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&output_path)
        .output()
        .expect("run compiled binary");
    assert_eq!(run.status.code(), Some(42));
}

#[test]
fn mo_build_compiles_and_runs_struct_fields() {
    let output_path = std::env::temp_dir().join(format!("mo_struct_fields_{}", std::process::id()));
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            "examples/compile/struct_fields.mo",
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");

    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&output_path)
        .output()
        .expect("run compiled binary");
    assert_eq!(run.status.code(), Some(42));
}

#[test]
fn mo_build_compiles_and_runs_print_int() {
    let output_path = std::env::temp_dir().join(format!("mo_print_int_{}", std::process::id()));
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            "examples/compile/print_int.mo",
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");

    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&output_path)
        .output()
        .expect("run compiled binary");
    assert_eq!(run.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&run.stdout), "42\n");
}

#[test]
fn mo_build_compiles_and_runs_normal_program() {
    let output_path =
        std::env::temp_dir().join(format!("mo_normal_program_{}", std::process::id()));
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            "examples/compile/normal_program.mo",
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");

    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&output_path)
        .output()
        .expect("run compiled binary");
    assert_eq!(run.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&run.stdout), "27\n");
}

#[test]
fn mo_build_compiles_and_runs_runtime_struct_string() {
    let output_path =
        std::env::temp_dir().join(format!("mo_runtime_struct_string_{}", std::process::id()));
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            "examples/compile/runtime_struct_string.mo",
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");

    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&output_path)
        .output()
        .expect("run compiled binary");
    assert_eq!(run.status.code(), Some(42));
}

#[test]
fn mo_build_compiles_and_runs_string_return() {
    let output_path = std::env::temp_dir().join(format!("mo_string_return_{}", std::process::id()));
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            "examples/compile/string_return.mo",
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");

    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&output_path)
        .output()
        .expect("run compiled binary");
    assert_eq!(run.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&run.stdout), "hello\n");
}

#[test]
fn mo_build_compiles_and_runs_std_string_new() {
    let output_path =
        std::env::temp_dir().join(format!("mo_std_string_new_{}", std::process::id()));
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            "examples/compile/std_string_new.mo",
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");

    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&output_path)
        .output()
        .expect("run compiled binary");
    assert_eq!(run.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&run.stdout), "hello\n");
}

#[test]
fn mo_build_compiles_and_runs_str_view_basic() {
    assert_eq!(
        build_and_run_exit_code("examples/compile/str_view_basic.mo", "mo_str_view_basic"),
        42
    );
}

#[test]
fn mo_build_compiles_and_runs_str_to_string_memory() {
    assert_eq!(
        build_and_run_exit_code(
            "examples/compile/str_to_string_memory.mo",
            "mo_str_to_string_memory"
        ),
        42
    );
}

#[test]
fn mo_build_compiles_and_runs_alloc_string_boundary() {
    assert_eq!(
        build_and_run_exit_code(
            "examples/compile/alloc_string_boundary.mo",
            "mo_alloc_string_boundary"
        ),
        42
    );
}

#[test]
fn mo_build_compiles_and_runs_alloc_buffer_boundary() {
    assert_eq!(
        build_and_run_exit_code(
            "examples/compile/alloc_buffer_boundary.mo",
            "mo_alloc_buffer_boundary"
        ),
        42
    );
}

#[test]
fn mo_build_compiles_and_runs_alloc_box_boundary() {
    assert_eq!(
        build_and_run_exit_code(
            "examples/compile/alloc_box_boundary.mo",
            "mo_alloc_box_boundary"
        ),
        42
    );
}

#[test]
fn mo_build_compiles_and_runs_alloc_vec_boundary() {
    assert_eq!(
        build_and_run_exit_code(
            "examples/compile/alloc_vec_boundary.mo",
            "mo_alloc_vec_boundary"
        ),
        42
    );
}

#[test]
fn mo_build_compiles_and_runs_alloc_map_boundary() {
    assert_eq!(
        build_and_run_exit_code(
            "examples/compile/alloc_map_boundary.mo",
            "mo_alloc_map_boundary"
        ),
        42
    );
}

#[test]
fn mo_build_compiles_and_runs_string_clone_memory() {
    assert_eq!(
        build_and_run_exit_code(
            "examples/compile/string_clone_memory.mo",
            "mo_string_clone_memory"
        ),
        42
    );
}

#[test]
fn mo_build_compiles_and_runs_string_param_drop_memory() {
    assert_eq!(
        build_and_run_exit_code(
            "examples/compile/string_param_drop_memory.mo",
            "mo_string_param_drop_memory"
        ),
        42
    );
}

#[test]
fn mo_build_compiles_and_runs_std_string_len() {
    assert_eq!(
        build_and_run_exit_code("examples/compile/std_string_len.mo", "mo_std_string_len"),
        5
    );
}

#[test]
fn mo_build_compiles_and_runs_runtime_memory_counters() {
    let source = std::env::temp_dir().join(format!(
        "mo_runtime_memory_counters_{}.mo",
        std::process::id()
    ));
    std::fs::write(
        &source,
        r#"
import * as core from "core/unsafe"

fn main() -> Int {
    let alloc0 = core.mem_alloc_count()
    let free0 = core.mem_free_count()
    let live0 = core.mem_live_bytes()
    let high0 = core.mem_high_water_bytes()
    let ptr = core.alloc(16)
    let alloc1 = core.mem_alloc_count()
    let live1 = core.mem_live_bytes()
    let high1 = core.mem_high_water_bytes()
    core.free(ptr)
    let free1 = core.mem_free_count()
    let live2 = core.mem_live_bytes()
    let high2 = core.mem_high_water_bytes()
    if alloc1 == alloc0 + 1 {
        if free1 == free0 + 1 {
            if live1 > live0 {
                if live2 == live0 {
                    if high1 >= live1 {
                        if high2 >= high0 {
                            return 42
                        }
                    }
                }
            }
        }
    }
    return 1
}
"#,
    )
    .expect("write memory counter source");

    assert_eq!(
        build_and_run_exit_code(
            source.to_str().expect("utf-8 path"),
            "mo_runtime_memory_counters"
        ),
        42
    );
}

#[test]
fn mo_build_compiles_and_runs_std_string_concat() {
    let source =
        std::env::temp_dir().join(format!("mo_std_string_concat_{}.mo", std::process::id()));
    std::fs::write(
        &source,
        r#"
import * as String from "std/string"
import * as core from "core/unsafe"

fn main() -> Int {
    let message = String.concat("hello, ", "world")
    return core.write(1, message)
}
"#,
    )
    .expect("write temp source");

    let output_path =
        std::env::temp_dir().join(format!("mo_std_string_concat_{}", std::process::id()));
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            source.to_str().expect("utf-8 path"),
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");

    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&output_path)
        .output()
        .expect("run compiled binary");
    assert_eq!(run.status.code(), Some(12));
    assert_eq!(String::from_utf8_lossy(&run.stdout), "hello, world");
}

#[test]
fn mo_build_compiles_and_runs_std_string_from_int() {
    let source =
        std::env::temp_dir().join(format!("mo_std_string_from_int_{}.mo", std::process::id()));
    std::fs::write(
        &source,
        r#"
import * as io from "std/io"
import * as String from "std/string"

fn main() -> Int {
    io.write_fd(1, String.from_int(42))
    return 0
}
"#,
    )
    .expect("write temp source");

    let output_path =
        std::env::temp_dir().join(format!("mo_std_string_from_int_{}", std::process::id()));
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            source.to_str().expect("utf-8 path"),
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");

    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&output_path)
        .output()
        .expect("run compiled binary");
    assert_eq!(run.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&run.stdout), "42");
}

#[test]
fn mo_build_compiles_and_runs_std_string_from_byte_and_json_punctuation() {
    let dir = std::env::temp_dir().join(format!("mo_std_string_from_byte_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp source dir");
    std::fs::write(dir.join("json.mo"), include_str!("../lib/json.mo"))
        .expect("write temp json module");
    let source = dir.join("main.mo");
    std::fs::write(
        &source,
        r#"
import * as bytes from "std/bytes"
import * as io from "std/io"
import * as json from "./json"
import * as String from "std/string"

fn main() -> Int {
    let text = json.encode_string("a_b!")
    io.write_fd(1, String.from_byte(bytes.string_load8(text, 2)))
    io.write_fd(1, String.from_byte(bytes.string_load8(text, 4)))
    return 0
}
"#,
    )
    .expect("write temp source");

    let output_path = dir.join("out");
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            source.to_str().expect("utf-8 path"),
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");

    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&output_path)
        .output()
        .expect("run compiled binary");
    assert_eq!(run.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&run.stdout), "_!");
}

#[test]
fn mo_build_compiles_and_runs_std_http_json_content_length_from_body() {
    let source =
        std::env::temp_dir().join(format!("mo_std_http_json_length_{}.mo", std::process::id()));
    std::fs::write(
        &source,
        r#"
import * as http from "std/http"

fn main() -> Int {
    http.write_json(1, "{\"x\":1}")
    return 0
}
"#,
    )
    .expect("write temp source");

    let output_path =
        std::env::temp_dir().join(format!("mo_std_http_json_length_{}", std::process::id()));
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            source.to_str().expect("utf-8 path"),
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");

    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&output_path)
        .output()
        .expect("run compiled binary");
    assert_eq!(run.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&run.stdout);
    assert!(stdout.contains("Content-Length: 7"), "{stdout}");
    assert!(stdout.ends_with("{\"x\":1}"), "{stdout}");
}

#[test]
fn mo_build_compiles_and_runs_raw_write_literal() {
    let output_path =
        std::env::temp_dir().join(format!("mo_raw_write_literal_{}", std::process::id()));
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            "examples/compile/raw_write_literal.mo",
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");

    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&output_path)
        .output()
        .expect("run compiled binary");
    assert_eq!(run.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&run.stdout), "hello\n");
}

#[test]
fn mo_build_compiles_and_runs_raw_write_string() {
    let output_path =
        std::env::temp_dir().join(format!("mo_raw_write_string_{}", std::process::id()));
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            "examples/compile/raw_write_string.mo",
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");

    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&output_path)
        .output()
        .expect("run compiled binary");
    assert_eq!(run.status.code(), Some(3));
    assert_eq!(String::from_utf8_lossy(&run.stdout), "ok\n");
}

#[test]
fn mo_build_compiles_and_runs_core_unsafe_memory_byte_roundtrip() {
    let source = std::env::temp_dir().join(format!("mo_core_unsafe_{}.mo", std::process::id()));
    std::fs::write(
        &source,
        r#"
import * as core from "core/unsafe"

fn main() -> Int {
    let ptr = core.alloc(4)
    core.store8(ptr, 0, 42)
    let value = core.load8(ptr, 0)
    core.free(ptr)
    return value
}
"#,
    )
    .expect("write temp source");

    let output_path = std::env::temp_dir().join(format!("mo_core_unsafe_{}", std::process::id()));
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            source.to_str().expect("utf-8 path"),
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");

    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&output_path)
        .output()
        .expect("run compiled binary");
    assert_eq!(run.status.code(), Some(42));
}

#[test]
fn mo_build_compiles_and_runs_core_unsafe_typed_ptr_roundtrip() {
    let code = build_and_run_exit_code(
        "examples/compile/core_unsafe_typed_ptr.mo",
        "mo_core_unsafe_typed_ptr",
    );
    assert_eq!(code, 42);
}

#[test]
fn mo_build_compiles_and_runs_core_unsafe_function_pointer_roundtrip() {
    let output_path = std::env::temp_dir().join(format!(
        "mo_function_pointer_roundtrip_{}",
        std::process::id()
    ));
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            "examples/compile/function_pointer_roundtrip.mo",
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");

    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&output_path)
        .output()
        .expect("run compiled binary");
    assert_eq!(run.status.code(), Some(42));
    assert_eq!(String::from_utf8_lossy(&run.stdout), "task\n");
}

#[test]
fn mo_build_emits_expression_statement_function_calls() {
    let source =
        std::env::temp_dir().join(format!("mo_expr_statement_call_{}.mo", std::process::id()));
    std::fs::write(
        &source,
        r#"
import * as core from "core/unsafe"

fn write_ready() -> Int {
    return core.write(1, "ready\n")
}

fn main() -> Int {
    write_ready()
    return 0
}
"#,
    )
    .expect("write temp source");

    let output_path =
        std::env::temp_dir().join(format!("mo_expr_statement_call_{}", std::process::id()));
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            source.to_str().expect("utf-8 path"),
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");

    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&output_path)
        .output()
        .expect("run compiled binary");
    assert_eq!(run.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&run.stdout), "ready\n");
}

#[test]
fn mo_build_compiles_and_runs_option_some_match() {
    let output_path = std::env::temp_dir().join(format!("mo_option_some_{}", std::process::id()));
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            "examples/compile/option_some.mo",
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");

    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&output_path)
        .output()
        .expect("run compiled binary");
    assert_eq!(run.status.code(), Some(42));
}

#[test]
fn mo_build_compiles_and_runs_option_none_match() {
    let output_path = std::env::temp_dir().join(format!("mo_option_none_{}", std::process::id()));
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            "examples/compile/option_none.mo",
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");

    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&output_path)
        .output()
        .expect("run compiled binary");
    assert_eq!(run.status.code(), Some(42));
}

#[test]
fn mo_build_compiles_and_runs_option_try_propagation() {
    assert_eq!(
        build_and_run_exit_code(
            "examples/compile/option_try_propagation.mo",
            "mo_option_try_propagation"
        ),
        42
    );
}

#[test]
fn mo_build_compiles_and_runs_boolean_logic_operators() {
    assert_eq!(
        build_and_run_exit_code("examples/compile/operator_bool_logic.mo", "mo_bool_logic"),
        42
    );
}

#[test]
fn mo_build_compiles_and_runs_bitwise_and_shift_operators() {
    assert_eq!(
        build_and_run_exit_code(
            "examples/compile/operator_bitwise_shift.mo",
            "mo_bitwise_shift"
        ),
        42
    );
}

#[test]
fn mo_build_compiles_and_runs_compound_assignment_operators() {
    assert_eq!(
        build_and_run_exit_code(
            "examples/compile/operator_compound_assignment.mo",
            "mo_compound_assignment"
        ),
        42
    );
}

#[test]
fn mo_build_compiles_and_runs_float64_arithmetic() {
    assert_eq!(
        build_and_run_exit_code(
            "examples/compile/float64_arithmetic.mo",
            "mo_float64_arithmetic"
        ),
        42
    );
}

#[test]
fn mo_build_compiles_and_runs_float64_struct_fields() {
    assert_eq!(
        build_and_run_exit_code(
            "examples/compile/float64_struct_fields.mo",
            "mo_float64_struct_fields"
        ),
        42
    );
}

#[test]
fn mo_build_compiles_and_runs_float64_compound_assignment() {
    assert_eq!(
        build_and_run_exit_code(
            "examples/compile/float64_compound_assignment.mo",
            "mo_float64_compound_assignment"
        ),
        42
    );
}

#[test]
fn mo_build_compiles_and_runs_float64_to_int() {
    assert_eq!(
        build_and_run_exit_code("examples/compile/float64_to_int.mo", "mo_float64_to_int"),
        42
    );
}

#[test]
fn mo_build_compiles_and_runs_result_ok_match_local() {
    assert_eq!(
        build_and_run_exit_code(
            "examples/compile/result_ok_match_local.mo",
            "mo_result_ok_match_local"
        ),
        42
    );
}

#[test]
fn mo_build_compiles_and_runs_result_err_match_local() {
    assert_eq!(
        build_and_run_exit_code(
            "examples/compile/result_err_match_local.mo",
            "mo_result_err_match_local"
        ),
        42
    );
}

#[test]
fn mo_build_compiles_and_runs_result_try_propagation() {
    assert_eq!(
        build_and_run_exit_code(
            "examples/compile/result_try_propagation.mo",
            "mo_result_try_propagation"
        ),
        42
    );
}

#[test]
fn mo_build_compiles_and_runs_general_enum_match() {
    assert_eq!(
        build_and_run_exit_code(
            "examples/compile/general_enum_match.mo",
            "mo_general_enum_match"
        ),
        42
    );
}

#[test]
fn mo_build_compiles_and_runs_multi_payload_enum_match() {
    assert_eq!(
        build_and_run_exit_code(
            "examples/compile/enum_multi_payload.mo",
            "mo_enum_multi_payload"
        ),
        42
    );
}

#[test]
fn mo_build_compiles_and_runs_statement_enum_match() {
    assert_eq!(
        build_and_run_exit_code(
            "examples/compile/statement_enum_match.mo",
            "mo_statement_enum_match"
        ),
        42
    );
}

#[test]
fn mo_build_compiles_and_runs_string_enum_match() {
    let output_path =
        std::env::temp_dir().join(format!("mo_enum_string_match_{}", std::process::id()));
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            "examples/compile/enum_string_match.mo",
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");

    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&output_path)
        .output()
        .expect("run compiled binary");
    assert_eq!(run.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&run.stdout), "hello\n");
}

#[test]
fn mo_build_compiles_and_runs_struct_enum_match() {
    let output_path =
        std::env::temp_dir().join(format!("mo_enum_struct_match_{}", std::process::id()));
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            "examples/compile/enum_struct_match.mo",
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");

    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&output_path)
        .output()
        .expect("run compiled binary");
    assert_eq!(run.status.code(), Some(42));
    assert_eq!(String::from_utf8_lossy(&run.stdout), "Ada\n");
}

#[test]
fn mo_build_compiles_and_runs_recursive_enum_drop() {
    assert_eq!(
        build_and_run_exit_code(
            "examples/compile/recursive_enum_drop.mo",
            "mo_recursive_enum_drop"
        ),
        42
    );
}

#[test]
fn mo_build_compiles_and_runs_recursive_owned_field_drop_memory() {
    assert_eq!(
        build_and_run_exit_code(
            "examples/compile/recursive_owned_field_drop_memory.mo",
            "mo_recursive_owned_field_drop_memory"
        ),
        42
    );
}

#[test]
fn mo_build_compiles_and_runs_generic_struct_string_drop_memory() {
    assert_eq!(
        build_and_run_exit_code(
            "examples/compile/generic_struct_string_drop_memory.mo",
            "mo_generic_struct_string_drop_memory"
        ),
        42
    );
}

#[test]
fn mo_build_compiles_and_runs_generic_struct_inferred_string_drop_memory() {
    assert_eq!(
        build_and_run_exit_code(
            "examples/compile/generic_struct_inferred_string_drop_memory.mo",
            "mo_generic_struct_inferred_string_drop_memory"
        ),
        42
    );
}

#[test]
fn mo_build_compiles_and_runs_generic_enum_string_drop_memory() {
    assert_eq!(
        build_and_run_exit_code(
            "examples/compile/generic_enum_string_drop_memory.mo",
            "mo_generic_enum_string_drop_memory"
        ),
        42
    );
}

#[test]
fn mo_build_compiles_and_runs_option_result_callback_owned_drop_memory() {
    assert_eq!(
        build_and_run_exit_code(
            "examples/compile/option_result_callback_owned_drop_memory.mo",
            "mo_option_result_callback_owned_drop_memory"
        ),
        42
    );
}

#[test]
fn mo_build_compiles_and_runs_result_or_else_owned_error_drop_memory() {
    assert_eq!(
        build_and_run_exit_code(
            "examples/compile/result_or_else_owned_error_drop_memory.mo",
            "mo_result_or_else_owned_error_drop_memory"
        ),
        42
    );
}

#[test]
fn mo_build_compiles_and_runs_destructured_relative_import() {
    let dir = std::env::temp_dir().join(format!("mo_import_destructure_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp package dir");
    std::fs::write(
        dir.join("math.mo"),
        r#"
pub fn add(a: Int, b: Int) -> Int {
    return a + b
}

fn ignored() -> Int {
    return 1
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

    let output_path = dir.join("main");
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            dir.join("main.mo").to_str().expect("utf-8 path"),
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");

    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&output_path)
        .output()
        .expect("run compiled binary");
    assert_eq!(run.status.code(), Some(42));
}

#[test]
fn mo_build_resolves_imports_relative_to_importing_file() {
    let dir = std::env::temp_dir().join(format!("mo_import_nested_{}", std::process::id()));
    let lib = dir.join("lib");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&lib).expect("create nested package dir");
    std::fs::write(
        lib.join("math.mo"),
        r#"
pub fn add(a: Int, b: Int) -> Int {
    return a + b
}
"#,
    )
    .expect("write nested math module");
    std::fs::write(
        lib.join("answer.mo"),
        r#"
import { add } from "./math"

pub fn answer() -> Int {
    return add(40, 2)
}
"#,
    )
    .expect("write nested answer module");
    std::fs::write(
        dir.join("main.mo"),
        r#"
import { answer } from "./lib/answer"

fn main() -> Int {
    return answer()
}
"#,
    )
    .expect("write main module");

    let output_path = dir.join("main");
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            dir.join("main.mo").to_str().expect("utf-8 path"),
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");

    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&output_path)
        .output()
        .expect("run compiled binary");
    assert_eq!(run.status.code(), Some(42));
}

#[test]
fn mo_build_resolves_manifest_package_imports() {
    let dir = std::env::temp_dir().join(format!("mo_manifest_import_{}", std::process::id()));
    let pkg = dir.join("packages").join("math");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&pkg).expect("create manifest package dir");
    std::fs::write(
        dir.join("mo.toml"),
        r#"
[package]
name = "app"
root = "."

[dependencies]
math = "packages/math"
"#,
    )
    .expect("write mo manifest");
    std::fs::write(
        pkg.join("answer.mo"),
        r#"
pub fn answer() -> Int {
    return 42
}
"#,
    )
    .expect("write manifest dependency module");
    std::fs::write(
        dir.join("main.mo"),
        r#"
import { answer } from "math/answer"

fn main() -> Int {
    return answer()
}
"#,
    )
    .expect("write manifest main module");

    let output_path = dir.join("main");
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            dir.join("main.mo").to_str().expect("utf-8 path"),
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");

    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&output_path)
        .output()
        .expect("run compiled binary");
    assert_eq!(run.status.code(), Some(42));
}

#[test]
fn mo_build_resolves_target_manifest_package_imports() {
    let dir =
        std::env::temp_dir().join(format!("mo_target_manifest_import_{}", std::process::id()));
    let pkg = dir.join("platform").join("math");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&pkg).expect("create target manifest package dir");
    std::fs::write(
        dir.join("mo.toml"),
        r#"
[package]
name = "app"
root = "."

[target.macos.dependencies]
math = "platform/math"
"#,
    )
    .expect("write target mo manifest");
    std::fs::write(
        pkg.join("answer.mo"),
        r#"
pub fn answer() -> Int {
    return 42
}
"#,
    )
    .expect("write target manifest dependency module");
    std::fs::write(
        dir.join("main.mo"),
        r#"
import { answer } from "math/answer"

fn main() -> Int {
    return answer()
}
"#,
    )
    .expect("write target manifest main module");

    let output_path = dir.join("main");
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            dir.join("main.mo").to_str().expect("utf-8 path"),
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");

    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&output_path)
        .output()
        .expect("run compiled binary");
    assert_eq!(run.status.code(), Some(42));
}

#[test]
fn mo_check_reports_missing_destructured_import() {
    let dir = std::env::temp_dir().join(format!("mo_import_missing_{}", std::process::id()));
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
import { subtract } from "./math"

fn main() -> Int {
    return 0
}
"#,
    )
    .expect("write main module");

    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args(["check", dir.join("main.mo").to_str().expect("utf-8 path")])
        .output()
        .expect("run mo check");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("does not export `subtract`"));
    assert!(stderr.contains("available public export(s): `add`"));
}

#[test]
fn mo_check_reports_private_destructured_import() {
    let dir = std::env::temp_dir().join(format!("mo_import_private_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp package dir");
    std::fs::write(
        dir.join("math.mo"),
        r#"
fn add(a: Int, b: Int) -> Int {
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

    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args(["check", dir.join("main.mo").to_str().expect("utf-8 path")])
        .output()
        .expect("run mo check");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("private item(s) `add`"));
    assert!(stderr.contains("add `pub` to export them"));
}

#[test]
fn mo_check_rejects_legacy_bare_glob_import() {
    let dir = std::env::temp_dir().join(format!("mo_import_glob_private_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp package dir");
    std::fs::write(
        dir.join("math.mo"),
        r#"
fn hidden() -> Int {
    return 40
}

pub fn visible() -> Int {
    return 2
}
"#,
    )
    .expect("write math module");
    std::fs::write(
        dir.join("main.mo"),
        r#"
import * from "./math"

fn main() -> Int {
    return 0
}
"#,
    )
    .expect("write main module");

    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args(["check", dir.join("main.mo").to_str().expect("utf-8 path")])
        .output()
        .expect("run mo check");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("unsupported import syntax"));
    assert!(stderr.contains("import * as name"));
}

#[test]
fn mo_build_compiles_and_runs_namespace_import() {
    let dir = std::env::temp_dir().join(format!("mo_import_namespace_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp package dir");
    std::fs::write(
        dir.join("math.mo"),
        r#"
fn hidden(a: Int) -> Int {
    return a + 1
}

pub fn add(a: Int, b: Int) -> Int {
    return hidden(a) + b
}
"#,
    )
    .expect("write math module");
    std::fs::write(
        dir.join("main.mo"),
        r#"
import * as math from "./math"

fn main() -> Int {
    return math.add(19, 22)
}
"#,
    )
    .expect("write main module");

    let output_path = dir.join("main");
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            dir.join("main.mo").to_str().expect("utf-8 path"),
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");

    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&output_path)
        .output()
        .expect("run compiled binary");
    assert_eq!(run.status.code(), Some(42));
}

#[test]
fn mo_build_compiles_and_runs_namespace_import_with_alias() {
    let dir =
        std::env::temp_dir().join(format!("mo_import_namespace_alias_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp package dir");
    std::fs::write(
        dir.join("pokemon_server.mo"),
        r#"
fn adjust(value: Int) -> Int {
    return value + 2
}

pub fn answer() -> Int {
    return adjust(40)
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

    let output_path = dir.join("main");
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            dir.join("main.mo").to_str().expect("utf-8 path"),
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");

    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&output_path)
        .output()
        .expect("run compiled binary");
    assert_eq!(run.status.code(), Some(42));
}

#[test]
fn mo_build_compiles_and_runs_lib_root_namespace_alias() {
    let source = std::env::temp_dir().join(format!(
        "mo_lib_root_namespace_alias_{}.mo",
        std::process::id()
    ));
    std::fs::write(
        &source,
        r#"
import * as encoder from "lib/json"
import * as String from "std/string"

fn main() -> Int {
    let field = encoder.field_int("level", 42)
    if String.len(field) > 0 {
        return 42
    }
    return 1
}
"#,
    )
    .expect("write temp source");

    let output_path = std::env::temp_dir().join(format!(
        "mo_lib_root_namespace_alias_{}",
        std::process::id()
    ));
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            source.to_str().expect("utf-8 path"),
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");

    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&output_path)
        .output()
        .expect("run compiled binary");
    assert_eq!(run.status.code(), Some(42));
}

#[test]
fn mo_check_reports_private_namespace_member() {
    let dir = std::env::temp_dir().join(format!(
        "mo_import_namespace_private_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp package dir");
    std::fs::write(
        dir.join("math.mo"),
        r#"
fn hidden() -> Int {
    return 42
}

pub fn visible() -> Int {
    return 1
}
"#,
    )
    .expect("write math module");
    std::fs::write(
        dir.join("main.mo"),
        r#"
import * as math from "./math"

fn main() -> Int {
    return math.hidden()
}
"#,
    )
    .expect("write main module");

    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args(["check", dir.join("main.mo").to_str().expect("utf-8 path")])
        .output()
        .expect("run mo check");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("is not exported by namespace `math`"));
    assert!(stderr.contains("available public export(s): `visible`"));
}

#[test]
fn mo_check_reports_private_namespace_member_with_alias() {
    let dir = std::env::temp_dir().join(format!(
        "mo_import_namespace_alias_private_{}",
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

    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args(["check", dir.join("main.mo").to_str().expect("utf-8 path")])
        .output()
        .expect("run mo check");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("is not exported by namespace `server`"));
    assert!(stderr.contains("available public export(s): `visible`"));
}

#[test]
fn mo_check_rejects_private_struct_field_read_across_modules() {
    let dir = std::env::temp_dir().join(format!(
        "mo_import_private_struct_field_read_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp package dir");
    std::fs::write(
        dir.join("user.mo"),
        r#"
pub struct User {
    pub id: Int
    name: Int
}

pub fn make() -> User {
    return User { id: 42, name: 7 }
}
"#,
    )
    .expect("write user module");
    std::fs::write(
        dir.join("main.mo"),
        r#"
import { User, make } from "./user"

fn main() -> Int {
    let user = make()
    return user.name
}
"#,
    )
    .expect("write main module");

    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args(["check", dir.join("main.mo").to_str().expect("utf-8 path")])
        .output()
        .expect("run mo check");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("field `name` on `User` is private"));
}

#[test]
fn mo_check_rejects_private_struct_field_literal_across_modules() {
    let dir = std::env::temp_dir().join(format!(
        "mo_import_private_struct_field_literal_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp package dir");
    std::fs::write(
        dir.join("user.mo"),
        r#"
pub struct User {
    pub id: Int
    name: Int
}
"#,
    )
    .expect("write user module");
    std::fs::write(
        dir.join("main.mo"),
        r#"
import { User } from "./user"

fn main() -> Int {
    let user = User { id: 42, name: 7 }
    return user.id
}
"#,
    )
    .expect("write main module");

    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args(["check", dir.join("main.mo").to_str().expect("utf-8 path")])
        .output()
        .expect("run mo check");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("field `name` on `User` is private"));
}

#[test]
fn mo_build_reads_public_struct_field_across_modules() {
    let dir = std::env::temp_dir().join(format!(
        "mo_import_public_struct_field_read_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp package dir");
    std::fs::write(
        dir.join("user.mo"),
        r#"
pub struct User {
    pub id: Int
    name: Int
}

pub fn make() -> User {
    return User { id: 42, name: 7 }
}
"#,
    )
    .expect("write user module");
    std::fs::write(
        dir.join("main.mo"),
        r#"
import { User, make } from "./user"

fn main() -> Int {
    let user = make()
    return user.id
}
"#,
    )
    .expect("write main module");

    let output_path = dir.join("main");
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            dir.join("main.mo").to_str().expect("utf-8 path"),
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");

    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&output_path)
        .output()
        .expect("run compiled binary");
    assert_eq!(run.status.code(), Some(42));
}

#[test]
fn mo_build_deduplicates_repeated_imports() {
    let dir = std::env::temp_dir().join(format!("mo_import_dedupe_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp package dir");
    std::fs::write(
        dir.join("math.mo"),
        r#"
pub fn answer() -> Int {
    return 42
}
"#,
    )
    .expect("write math module");
    std::fs::write(
        dir.join("left.mo"),
        r#"
import { answer } from "./math"

pub fn left() -> Int {
    return answer()
}
"#,
    )
    .expect("write left module");
    std::fs::write(
        dir.join("right.mo"),
        r#"
import { answer } from "./math"

pub fn right() -> Int {
    return answer()
}
"#,
    )
    .expect("write right module");
    std::fs::write(
        dir.join("main.mo"),
        r#"
import { left } from "./left"
import { right } from "./right"

fn main() -> Int {
    return left() + right() - 42
}
"#,
    )
    .expect("write main module");

    let output_path = dir.join("main");
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            dir.join("main.mo").to_str().expect("utf-8 path"),
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");

    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&output_path)
        .output()
        .expect("run compiled binary");
    assert_eq!(run.status.code(), Some(42));
}

#[test]
fn mo_check_discovers_std_package_modules() {
    let source = std::env::temp_dir().join(format!("mo_std_import_{}.mo", std::process::id()));
    std::fs::write(
        &source,
        r#"
import { Request, Response, Server } from "std/http"
import * as fs from "std/fs"
import * as net from "std/net"

fn main() -> Int {
    return 0
}
"#,
    )
    .expect("write temp source");

    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args(["check", source.to_str().expect("utf-8 path")])
        .output()
        .expect("run mo check");

    assert!(
        output.status.success(),
        "mo check failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn mo_build_emits_module_qualified_object_symbols() {
    let dir = std::env::temp_dir().join(format!("mo_object_symbols_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp package dir");
    std::fs::write(
        dir.join("math.mo"),
        r#"
module app.math

pub fn add(a: Int, b: Int) -> Int {
    return a + b
}
"#,
    )
    .expect("write math module");
    std::fs::write(
        dir.join("main.mo"),
        r#"
module app.main

import { add } from "./math"

fn main() -> Int {
    return add(20, 22)
}
"#,
    )
    .expect("write main module");

    let output_path = dir.join("main");
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            dir.join("main.mo").to_str().expect("utf-8 path"),
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");

    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let object_path = output_path.with_extension("o");
    let symbols = Command::new("nm")
        .arg(&object_path)
        .output()
        .expect("run nm");
    assert!(
        symbols.status.success(),
        "nm failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&symbols.stdout),
        String::from_utf8_lossy(&symbols.stderr)
    );
    let stdout = String::from_utf8_lossy(&symbols.stdout);
    assert!(stdout.contains("__mo_app__math__add"), "{stdout}");
}

#[test]
fn mo_build_compiles_and_runs_extern_getpid_call() {
    let source = std::env::temp_dir().join(format!("mo_extern_getpid_{}.mo", std::process::id()));
    std::fs::write(
        &source,
        r#"
extern "C" {
    fn getpid() -> Int32
}

fn main() -> Int {
    if getpid() > 0 {
        return 42
    }
    return 1
}
"#,
    )
    .expect("write temp source");

    let output_path = std::env::temp_dir().join(format!("mo_extern_getpid_{}", std::process::id()));
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            source.to_str().expect("utf-8 path"),
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");

    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&output_path)
        .output()
        .expect("run compiled binary");
    assert_eq!(run.status.code(), Some(42));
}

#[test]
fn mo_build_links_static_library_from_dependency_manifest() {
    let root = std::env::temp_dir().join(format!("mo_native_static_pkg_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let app_dir = root.join("app");
    let pkg_dir = app_dir.join("packages/native_answer");
    let src_dir = pkg_dir.join("src");
    let vendor_dir = pkg_dir.join("vendor");
    std::fs::create_dir_all(&src_dir).expect("create package src");
    std::fs::create_dir_all(&vendor_dir).expect("create vendor");

    let c_path = vendor_dir.join("answer.c");
    let o_path = vendor_dir.join("answer.o");
    let a_path = vendor_dir.join("libanswer.a");
    std::fs::write(&c_path, "int answer_from_c(void) { return 42; }\n").expect("write c source");
    let cc = Command::new("cc")
        .args([
            "-c",
            c_path.to_str().expect("utf-8 path"),
            "-o",
            o_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("compile c object");
    assert!(
        cc.status.success(),
        "cc failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&cc.stdout),
        String::from_utf8_lossy(&cc.stderr)
    );
    let ar = Command::new("ar")
        .args([
            "rcs",
            a_path.to_str().expect("utf-8 path"),
            o_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("archive static library");
    assert!(
        ar.status.success(),
        "ar failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&ar.stdout),
        String::from_utf8_lossy(&ar.stderr)
    );

    std::fs::write(
        pkg_dir.join("mo.toml"),
        r#"
[package]
name = "native_answer"
root = "src"

[native.macos.aarch64]
static_libraries = ["vendor/libanswer.a"]
"#,
    )
    .expect("write package manifest");
    std::fs::write(
        src_dir.join("answer.mo"),
        r#"
extern "C" {
    fn answer_from_c() -> Int32
}

pub fn answer() -> Int {
    return answer_from_c()
}
"#,
    )
    .expect("write package source");
    std::fs::write(
        app_dir.join("mo.toml"),
        r#"
[dependencies]
native_answer = "packages/native_answer/src"
"#,
    )
    .expect("write app manifest");
    let app_source = app_dir.join("main.mo");
    std::fs::write(
        &app_source,
        r#"
import * as answer from "native_answer/answer"

fn main() -> Int {
    return answer.answer()
}
"#,
    )
    .expect("write app source");

    let output_path = root.join("app_bin");
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            app_source.to_str().expect("utf-8 path"),
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");
    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let run = Command::new(&output_path)
        .output()
        .expect("run compiled binary");
    assert_eq!(run.status.code(), Some(42));
}

#[test]
fn mo_build_links_object_from_dependency_manifest() {
    let root = std::env::temp_dir().join(format!("mo_native_object_pkg_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let app_dir = root.join("app");
    let pkg_dir = app_dir.join("packages/native_answer");
    let src_dir = pkg_dir.join("src");
    let vendor_dir = pkg_dir.join("vendor");
    std::fs::create_dir_all(&src_dir).expect("create package src");
    std::fs::create_dir_all(&vendor_dir).expect("create vendor");

    let c_path = vendor_dir.join("answer.c");
    let o_path = vendor_dir.join("answer.o");
    std::fs::write(&c_path, "int answer_from_object(void) { return 42; }\n")
        .expect("write c source");
    let cc = Command::new("cc")
        .args([
            "-c",
            c_path.to_str().expect("utf-8 path"),
            "-o",
            o_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("compile c object");
    assert!(
        cc.status.success(),
        "cc failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&cc.stdout),
        String::from_utf8_lossy(&cc.stderr)
    );

    std::fs::write(
        pkg_dir.join("mo.toml"),
        r#"
[package]
name = "native_answer"
root = "src"

[native.macos.aarch64]
objects = ["vendor/answer.o"]
"#,
    )
    .expect("write package manifest");
    std::fs::write(
        src_dir.join("answer.mo"),
        r#"
extern "C" {
    fn answer_from_object() -> Int32
}

pub fn answer() -> Int {
    return answer_from_object()
}
"#,
    )
    .expect("write package source");
    std::fs::write(
        app_dir.join("mo.toml"),
        r#"
[dependencies]
native_answer = "packages/native_answer/src"
"#,
    )
    .expect("write app manifest");
    let app_source = app_dir.join("main.mo");
    std::fs::write(
        &app_source,
        r#"
import * as answer from "native_answer/answer"

fn main() -> Int {
    return answer.answer()
}
"#,
    )
    .expect("write app source");

    let output_path = root.join("app_bin");
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            app_source.to_str().expect("utf-8 path"),
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");
    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let run = Command::new(&output_path)
        .output()
        .expect("run compiled binary");
    assert_eq!(run.status.code(), Some(42));
}

#[test]
fn mo_build_reports_missing_native_static_library() {
    let root = std::env::temp_dir().join(format!("mo_native_missing_pkg_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let app_dir = root.join("app");
    let pkg_dir = app_dir.join("packages/native_answer");
    let src_dir = pkg_dir.join("src");
    std::fs::create_dir_all(&src_dir).expect("create package src");

    std::fs::write(
        pkg_dir.join("mo.toml"),
        r#"
[package]
name = "native_answer"
root = "src"

[native.macos.aarch64]
static_libraries = ["vendor/missing.a"]
"#,
    )
    .expect("write package manifest");
    std::fs::write(
        src_dir.join("answer.mo"),
        r#"
extern "C" {
    fn answer_from_c() -> Int32
}

pub fn answer() -> Int {
    return answer_from_c()
}
"#,
    )
    .expect("write package source");
    std::fs::write(
        app_dir.join("mo.toml"),
        r#"
[dependencies]
native_answer = "packages/native_answer/src"
"#,
    )
    .expect("write app manifest");
    let app_source = app_dir.join("main.mo");
    std::fs::write(
        &app_source,
        r#"
import * as answer from "native_answer/answer"

fn main() -> Int {
    return answer.answer()
}
"#,
    )
    .expect("write app source");

    let output_path = root.join("app_bin");
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            app_source.to_str().expect("utf-8 path"),
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("native static library not found"),
        "stderr did not include missing native library diagnostic:\n{stderr}"
    );
}

#[test]
fn mo_exec_runs_manifest_script_from_manifest_directory() {
    let root = std::env::temp_dir().join(format!("mo_exec_script_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create project dir");
    std::fs::write(
        root.join("mo.toml"),
        r#"
[scripts]
write = "pwd > script.cwd"
"#,
    )
    .expect("write manifest");

    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args(["exec", "write", root.to_str().expect("utf-8 path")])
        .output()
        .expect("run mo exec");

    assert!(
        output.status.success(),
        "mo exec failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let cwd = std::fs::read_to_string(root.join("script.cwd")).expect("read script cwd");
    let expected = std::fs::canonicalize(&root).expect("canonicalize project dir");
    assert_eq!(cwd.trim(), expected.to_string_lossy());
}

#[test]
fn mo_exec_reports_missing_script_names() {
    let root = std::env::temp_dir().join(format!("mo_exec_missing_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create project dir");
    std::fs::write(
        root.join("mo.toml"),
        r#"
[scripts]
build = "printf ok"
"#,
    )
    .expect("write manifest");

    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args(["exec", "run", root.to_str().expect("utf-8 path")])
        .output()
        .expect("run mo exec");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("script `run` not found") && stderr.contains("available scripts: build"),
        "stderr did not include missing script diagnostic:\n{stderr}"
    );
}

#[test]
fn mo_build_compiles_and_runs_socket_create_close_smoke() {
    let source = std::env::temp_dir().join(format!("mo_socket_smoke_{}.mo", std::process::id()));
    std::fs::write(
        &source,
        r#"
extern "C" {
    fn socket(domain: Int32, kind: Int32, protocol: Int32) -> Int32
    fn close(fd: Int32) -> Int32
}

fn main() -> Int {
    let fd = socket(2, 1, 0)
    if fd > 0 {
        close(fd)
        return 42
    }
    return 1
}
"#,
    )
    .expect("write temp source");

    let output_path = std::env::temp_dir().join(format!("mo_socket_smoke_{}", std::process::id()));
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            source.to_str().expect("utf-8 path"),
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");

    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&output_path)
        .output()
        .expect("run compiled binary");
    assert_eq!(run.status.code(), Some(42));
}

#[test]
fn mo_build_compiles_and_runs_socket_listen_close_smoke() {
    let source = std::env::temp_dir().join(format!("mo_socket_listen_{}.mo", std::process::id()));
    std::fs::write(
        &source,
        r#"
extern "C" {
    fn socket(domain: Int32, kind: Int32, protocol: Int32) -> Int32
    fn listen(fd: Int32, backlog: Int32) -> Int32
    fn close(fd: Int32) -> Int32
}

fn main() -> Int {
    let fd = socket(2, 1, 0)
    if fd > 0 {
        let ok = listen(fd, 16)
        close(fd)
        if ok == 0 {
            return 42
        }
    }
    return 1
}
"#,
    )
    .expect("write temp source");

    let output_path = std::env::temp_dir().join(format!("mo_socket_listen_{}", std::process::id()));
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            source.to_str().expect("utf-8 path"),
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");

    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&output_path)
        .output()
        .expect("run compiled binary");
    assert_eq!(run.status.code(), Some(42));
}

#[test]
fn mo_build_compiles_and_runs_socket_bind_listen_close_smoke() {
    let source = std::env::temp_dir().join(format!("mo_socket_bind_{}.mo", std::process::id()));
    std::fs::write(
        &source,
        r#"
extern "C" {
    fn socket(domain: Int32, kind: Int32, protocol: Int32) -> Int32
    fn bind(fd: Int32, addr: Int, len: Int32) -> Int32
    fn listen(fd: Int32, backlog: Int32) -> Int32
    fn close(fd: Int32) -> Int32
}

import * as core from "core/unsafe"

fn zero_sockaddr_in(addr: Int) {
    core.store8(addr, 0, 0)
    core.store8(addr, 1, 0)
    core.store8(addr, 2, 0)
    core.store8(addr, 3, 0)
    core.store8(addr, 4, 0)
    core.store8(addr, 5, 0)
    core.store8(addr, 6, 0)
    core.store8(addr, 7, 0)
    core.store8(addr, 8, 0)
    core.store8(addr, 9, 0)
    core.store8(addr, 10, 0)
    core.store8(addr, 11, 0)
    core.store8(addr, 12, 0)
    core.store8(addr, 13, 0)
    core.store8(addr, 14, 0)
    core.store8(addr, 15, 0)
}

fn main() -> Int {
    let fd = socket(2, 1, 0)
    if fd > 0 {
        let addr = core.alloc(16)
        zero_sockaddr_in(addr)
        core.store8(addr, 0, 16)
        core.store8(addr, 1, 2)

        let bind_ok = bind(fd, addr, 16)
        core.free(addr)

        if bind_ok == 0 {
            let listen_ok = listen(fd, 16)
            close(fd)
            if listen_ok == 0 {
                return 42
            }
            return 2
        }
        close(fd)
        return 3
    }
    return 1
}
"#,
    )
    .expect("write temp source");

    let output_path = std::env::temp_dir().join(format!("mo_socket_bind_{}", std::process::id()));
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            source.to_str().expect("utf-8 path"),
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");

    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&output_path)
        .output()
        .expect("run compiled binary");
    assert_eq!(run.status.code(), Some(42));
}

#[test]
fn mo_build_compiles_and_runs_std_net_tcp_listen_ephemeral_smoke() {
    let source = std::env::temp_dir().join(format!("mo_std_net_listen_{}.mo", std::process::id()));
    std::fs::write(
        &source,
        r#"
import * as net from "std/net"

fn main() -> Int {
    let fd = net.tcp_listen_ephemeral(16)
    if fd > 0 {
        net.close_fd(fd)
        return 42
    }
    return 1
}
"#,
    )
    .expect("write temp source");

    let output_path =
        std::env::temp_dir().join(format!("mo_std_net_listen_{}", std::process::id()));
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            source.to_str().expect("utf-8 path"),
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");

    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&output_path)
        .output()
        .expect("run compiled binary");
    assert_eq!(run.status.code(), Some(42));
}

#[test]
fn mo_build_compiles_and_runs_std_net_nonblocking_listener_smoke() {
    let source =
        std::env::temp_dir().join(format!("mo_std_net_nonblocking_{}.mo", std::process::id()));
    std::fs::write(
        &source,
        r#"
import * as net from "std/net"

fn main() -> Int {
    let fd = net.tcp_listen_ephemeral(16)
    if fd > 0 {
        let ok = net.set_nonblocking(fd)
        net.close_fd(fd)
        if ok == 0 {
            return 42
        }
        return 2
    }
    return 1
}
"#,
    )
    .expect("write temp source");

    let output_path =
        std::env::temp_dir().join(format!("mo_std_net_nonblocking_{}", std::process::id()));
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            source.to_str().expect("utf-8 path"),
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");

    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&output_path)
        .output()
        .expect("run compiled binary");
    assert_eq!(run.status.code(), Some(42));
}

#[test]
fn mo_build_compiles_and_runs_std_net_nonblocking_accept_no_client_smoke() {
    let source = std::env::temp_dir().join(format!("mo_std_net_accept_{}.mo", std::process::id()));
    std::fs::write(
        &source,
        r#"
import * as net from "std/net"

fn main() -> Int {
    let fd = net.tcp_listen_ephemeral(16)
    if fd > 0 {
        let nonblocking = net.set_nonblocking(fd)
        if nonblocking == 0 {
            let client = net.accept_fd(fd)
            net.close_fd(fd)
            if client < 0 {
                return 42
            }
            net.close_fd(client)
            return 2
        }
        net.close_fd(fd)
        return 3
    }
    return 1
}
"#,
    )
    .expect("write temp source");

    let output_path =
        std::env::temp_dir().join(format!("mo_std_net_accept_{}", std::process::id()));
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            source.to_str().expect("utf-8 path"),
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");

    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&output_path)
        .output()
        .expect("run compiled binary");
    assert_eq!(run.status.code(), Some(42));
}

#[test]
fn mo_build_compiles_and_runs_std_net_tcp_loopback_byte_roundtrip() {
    let source =
        std::env::temp_dir().join(format!("mo_std_net_loopback_{}.mo", std::process::id()));
    std::fs::write(
        &source,
        r#"
import * as io from "std/io"
import * as net from "std/net"

fn main() -> Int {
    let listener = net.tcp_listen_ephemeral(16)
    if listener > 0 {
        let port = net.listener_port(listener)
        if port > 0 {
            let client = net.tcp_connect_loopback(port)
            if client > 0 {
                let server = net.accept_fd(listener)
                if server > 0 {
                    let wrote = io.write_fd(client, "A")
                    if wrote == 1 {
                        let byte = io.read_byte_fd(server)
                        net.close_fd(server)
                        net.close_fd(client)
                        net.close_fd(listener)
                        if byte == 65 {
                            return 42
                        }
                        return 5
                    }
                    net.close_fd(server)
                    net.close_fd(client)
                    net.close_fd(listener)
                    return 4
                }
                net.close_fd(client)
                net.close_fd(listener)
                return 3
            }
            net.close_fd(listener)
            return 2
        }
        net.close_fd(listener)
        return 1
    }
    return 6
}
"#,
    )
    .expect("write temp source");

    let output_path =
        std::env::temp_dir().join(format!("mo_std_net_loopback_{}", std::process::id()));
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            source.to_str().expect("utf-8 path"),
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");

    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&output_path)
        .output()
        .expect("run compiled binary");
    assert_eq!(run.status.code(), Some(42));
}

#[test]
fn mo_build_compiles_and_runs_std_net_tcp_wrapper_loopback_byte_roundtrip() {
    let source = std::env::temp_dir().join(format!(
        "mo_std_net_wrapper_loopback_{}.mo",
        std::process::id()
    ));
    std::fs::write(
        &source,
        r#"
import * as net from "std/net"

fn main() -> Int {
    let listener = net.listener_new(16)
    let port = net.listener_port(listener)
    if port > 0 {
        let client = net.stream_connect_loopback(port)
        let server = net.listener_accept(listener)
        let wrote = net.stream_write(client, "A")
        if wrote == 1 {
            let byte = net.stream_read_byte(server)
            net.stream_close(server)
            net.stream_close(client)
            net.listener_close(listener)
            if byte == 65 {
                return 42
            }
            return 4
        }
        net.stream_close(server)
        net.stream_close(client)
        net.listener_close(listener)
        return 3
    }
    net.listener_close(listener)
    return 1
}
"#,
    )
    .expect("write temp source");

    let output_path = std::env::temp_dir().join(format!(
        "mo_std_net_wrapper_loopback_{}",
        std::process::id()
    ));
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            source.to_str().expect("utf-8 path"),
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");

    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&output_path)
        .output()
        .expect("run compiled binary");
    assert_eq!(run.status.code(), Some(42));
}

#[test]
fn mo_build_compiles_and_runs_std_net_tcp_typed_wrapper_loopback_byte_roundtrip() {
    let code = build_and_run_exit_code(
        "examples/compile/std_net_tcp_typed_wrapper_loopback.mo",
        "mo_std_net_tcp_typed_wrapper_loopback",
    );
    assert_eq!(code, 42);
}

#[test]
fn mo_build_compiles_and_runs_std_event_loop_select() {
    let code = build_and_run_exit_code(
        "examples/compile/std_event_loop_select.mo",
        "mo_std_event_loop_select",
    );
    assert_eq!(code, 42);
}

#[test]
fn mo_build_compiles_and_runs_std_async_tcp_loopback() {
    let code = build_and_run_exit_code(
        "examples/compile/std_async_tcp_loopback.mo",
        "mo_std_async_tcp_loopback",
    );
    assert_eq!(code, 42);
}

#[test]
fn mo_build_compiles_and_runs_std_net_wait_readable_listener_smoke() {
    let source = std::env::temp_dir().join(format!(
        "mo_std_net_wait_readable_{}.mo",
        std::process::id()
    ));
    std::fs::write(
        &source,
        r#"
import * as net from "std/net"

fn main() -> Int {
    let listener = net.tcp_listen_ephemeral(16)
    if listener > 0 {
        let port = net.listener_port(listener)
        if port > 0 {
            let client = net.tcp_connect_loopback(port)
            if client > 0 {
                let ready = net.wait_readable(listener)
                if ready > 0 {
                    let server = net.accept_fd(listener)
                    if server > 0 {
                        net.close_fd(server)
                        net.close_fd(client)
                        net.close_fd(listener)
                        return 42
                    }
                    net.close_fd(client)
                    net.close_fd(listener)
                    return 4
                }
                net.close_fd(client)
                net.close_fd(listener)
                return 3
            }
            net.close_fd(listener)
            return 2
        }
        net.close_fd(listener)
        return 1
    }
    return 5
}
"#,
    )
    .expect("write temp source");

    let output_path =
        std::env::temp_dir().join(format!("mo_std_net_wait_readable_{}", std::process::id()));
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            source.to_str().expect("utf-8 path"),
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");

    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&output_path)
        .output()
        .expect("run compiled binary");
    assert_eq!(run.status.code(), Some(42));
}

#[test]
fn mo_build_compiles_and_runs_std_http_write_ok_over_tcp() {
    let source = std::env::temp_dir().join(format!("mo_std_http_ok_{}.mo", std::process::id()));
    std::fs::write(
        &source,
        r#"
import * as http from "std/http"
import * as io from "std/io"
import * as net from "std/net"

fn main() -> Int {
    let listener = net.tcp_listen_ephemeral(16)
    if listener > 0 {
        let port = net.listener_port(listener)
        if port > 0 {
            let client = net.tcp_connect_loopback(port)
            if client > 0 {
                net.wait_readable(listener)
                let server = net.accept_fd(listener)
                if server > 0 {
                    let wrote = http.write_ok(server)
                    if wrote > 0 {
                        let first = io.read_byte_fd(client)
                        net.close_fd(server)
                        net.close_fd(client)
                        net.close_fd(listener)
                        if first == 72 {
                            return 42
                        }
                        return 5
                    }
                    net.close_fd(server)
                    net.close_fd(client)
                    net.close_fd(listener)
                    return 4
                }
                net.close_fd(client)
                net.close_fd(listener)
                return 3
            }
            net.close_fd(listener)
            return 2
        }
        net.close_fd(listener)
        return 1
    }
    return 6
}
"#,
    )
    .expect("write temp source");

    let output_path = std::env::temp_dir().join(format!("mo_std_http_ok_{}", std::process::id()));
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            source.to_str().expect("utf-8 path"),
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");

    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&output_path)
        .output()
        .expect("run compiled binary");
    assert_eq!(run.status.code(), Some(42));
}

#[test]
fn mo_build_compiles_and_runs_std_http_reads_get_request_over_tcp() {
    let source = std::env::temp_dir().join(format!("mo_std_http_get_{}.mo", std::process::id()));
    std::fs::write(
        &source,
        r#"
import * as http from "std/http"
import * as io from "std/io"
import * as net from "std/net"

fn main() -> Int {
    let listener = net.tcp_listen_ephemeral(16)
    if listener > 0 {
        let port = net.listener_port(listener)
        if port > 0 {
            let client = net.tcp_connect_loopback(port)
            if client > 0 {
                let request_written = io.write_fd(client, "GET / HTTP/1.1\r\nHost: localhost\r\n\r\n")
                if request_written > 0 {
                    net.wait_readable(listener)
                    let server = net.accept_fd(listener)
                    if server > 0 {
                        let is_get = http.request_starts_get(server)
                        if is_get == 1 {
                            let response_written = http.write_ok(server)
                            if response_written > 0 {
                                let first = io.read_byte_fd(client)
                                net.close_fd(server)
                                net.close_fd(client)
                                net.close_fd(listener)
                                if first == 72 {
                                    return 42
                                }
                                return 6
                            }
                            return 6
                        }
                        net.close_fd(server)
                        net.close_fd(client)
                        net.close_fd(listener)
                        return 5
                    }
                    net.close_fd(client)
                    net.close_fd(listener)
                    return 4
                }
                net.close_fd(client)
                net.close_fd(listener)
                return 3
            }
            net.close_fd(listener)
            return 2
        }
        net.close_fd(listener)
        return 1
    }
    return 7
}
"#,
    )
    .expect("write temp source");

    let output_path = std::env::temp_dir().join(format!("mo_std_http_get_{}", std::process::id()));
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            source.to_str().expect("utf-8 path"),
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");

    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&output_path)
        .output()
        .expect("run compiled binary");
    assert_eq!(run.status.code(), Some(42));
}

#[test]
fn mo_build_compiles_and_runs_std_fs_process_path_smoke() {
    assert_eq!(
        build_and_run_exit_code(
            "examples/compile/fs_process_path_smoke.mo",
            "mo_std_fs_process_path"
        ),
        42
    );
}

#[test]
fn mo_build_compiles_and_runs_std_fs_rewrite_after_read() {
    assert_eq!(
        build_and_run_exit_code(
            "examples/compile/fs_read_then_write.mo",
            "mo_std_fs_read_then_write"
        ),
        42
    );
    assert_eq!(
        build_and_run_exit_code(
            "examples/compile/fs_open_close_then_write.mo",
            "mo_std_fs_open_close_then_write"
        ),
        42
    );
}

#[test]
fn mo_build_compiles_and_runs_userland_pokemon_json() {
    assert_eq!(
        build_and_run_exit_code(
            "examples/compile/pokemon_json.mo",
            "mo_userland_pokemon_json"
        ),
        42
    );
}

#[test]
fn mo_build_compiles_and_runs_pokemon_rest_api() {
    assert_eq!(
        build_and_run_exit_code(
            "examples/compile/pokemon_rest_api.mo",
            "mo_pokemon_rest_api"
        ),
        42
    );
}

#[test]
fn mo_build_compiles_and_runs_express_route_table_middleware() {
    assert_eq!(
        build_and_run_exit_code(
            "examples/compile/express_route_table_middleware.mo",
            "mo_express_route_table_middleware"
        ),
        42
    );
}

#[test]
fn mo_build_compiles_and_runs_express_dynamic_route() {
    assert_eq!(
        build_and_run_exit_code(
            "examples/compile/express_dynamic_route.mo",
            "mo_express_dynamic_route"
        ),
        42
    );
}

#[test]
fn mo_build_compiles_and_runs_async_spawn_join() {
    assert_eq!(
        build_and_run_exit_code(
            "examples/compile/async_spawn_join.mo",
            "mo_async_spawn_join"
        ),
        42
    );
}

#[test]
fn mo_build_compiles_and_runs_async_await_block_on() {
    assert_eq!(
        build_and_run_exit_code(
            "examples/compile/async_await_block_on.mo",
            "mo_async_await_block_on"
        ),
        42
    );
}

#[test]
fn mo_build_compiles_and_runs_pokemon_threadpool_rest_api() {
    assert_eq!(
        build_and_run_exit_code(
            "examples/compile/pokemon_threadpool_rest_api.mo",
            "mo_pokemon_threadpool_rest_api"
        ),
        42
    );
}

#[test]
fn mo_build_compiles_and_runs_pokemon_threadpool_memory() {
    assert_eq!(
        build_and_run_exit_code(
            "examples/compile/pokemon_threadpool_memory.mo",
            "mo_pokemon_threadpool_memory"
        ),
        42
    );
}

#[test]
fn mo_build_compiles_and_runs_express_threadpool_dynamic_route() {
    assert_eq!(
        build_and_run_exit_code(
            "examples/compile/express_threadpool_dynamic_route.mo",
            "mo_express_threadpool_dynamic_route"
        ),
        42
    );
}

#[test]
fn mo_build_compiles_and_runs_std_io_pipe_byte_roundtrip() {
    let source = std::env::temp_dir().join(format!("mo_std_io_pipe_{}.mo", std::process::id()));
    std::fs::write(
        &source,
        r#"
import * as io from "std/io"
import * as core from "core/unsafe"
import * as bytes from "std/bytes"

extern "C" {
    fn pipe(fds: Int) -> Int32
}

fn fd_at(fds: Int, slot: Int) -> Int {
    let offset = slot * 4
    return bytes.load_u32_le(fds, offset)
}

fn main() -> Int {
    let fds = core.alloc(8)
    let ok = pipe(fds)
    if ok == 0 {
        let read_fd = fd_at(fds, 0)
        let write_fd = fd_at(fds, 1)
        let wrote = io.write_fd(write_fd, "A")
        if wrote != 1 {
            io.close_fd(read_fd)
            io.close_fd(write_fd)
            core.free(fds)
            return 2
        }
        let byte = io.read_byte_fd(read_fd)
        io.close_fd(read_fd)
        io.close_fd(write_fd)
        core.free(fds)
        if byte == 65 {
            return 42
        }
        return 3
    }
    core.free(fds)
    return 1
}
"#,
    )
    .expect("write temp source");

    let output_path = std::env::temp_dir().join(format!("mo_std_io_pipe_{}", std::process::id()));
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            source.to_str().expect("utf-8 path"),
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");

    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&output_path)
        .output()
        .expect("run compiled binary");
    assert_eq!(run.status.code(), Some(42));
}

#[test]
fn mo_build_compiles_and_runs_std_int_decimal_parse_and_format() {
    let source = std::env::temp_dir().join(format!("mo_std_int_{}.mo", std::process::id()));
    std::fs::write(
        &source,
        r#"
import * as int from "std/int"
import * as io from "std/io"

fn main() -> Int {
    let parsed = int.parse_decimal("123")
    let negative = int.parse_decimal("-5")
    let fallback = int.parse_decimal_or("12x", 7)
    io.write_fd(1, int.to_string(parsed + negative + fallback))
    return parsed + negative + fallback
}
"#,
    )
    .expect("write temp source");

    let output_path = std::env::temp_dir().join(format!("mo_std_int_{}", std::process::id()));
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            source.to_str().expect("utf-8 path"),
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");

    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&output_path)
        .output()
        .expect("run compiled binary");
    assert_eq!(run.status.code(), Some(125));
    assert_eq!(String::from_utf8_lossy(&run.stdout), "125");
}

#[test]
fn mo_build_compiles_and_runs_std_bytes_digit_and_endian_helpers() {
    let source = std::env::temp_dir().join(format!("mo_std_bytes_{}.mo", std::process::id()));
    std::fs::write(
        &source,
        r#"
import * as bytes from "std/bytes"
import * as core from "core/unsafe"

fn main() -> Int {
    let ptr = core.alloc(4)
    bytes.store_u16_be(ptr, 0, 4660)
    bytes.store_u16_le(ptr, 2, 52)
    let first = bytes.load_u16_be(ptr, 0)
    let second = bytes.load_u16_le(ptr, 2)
    bytes.store_u32_le(ptr, 0, 16909060)
    let word = bytes.load_u32_le(ptr, 0)
    core.free(ptr)

    if bytes.digit_value(55) == 7 {
        if bytes.digit_value(65) == 0 - 1 {
            if first == 4660 {
                if second == 52 {
                    if word == 16909060 {
                        return 42
                    }
                }
            }
        }
    }
    return 1
}
"#,
    )
    .expect("write temp source");

    let output_path = std::env::temp_dir().join(format!("mo_std_bytes_{}", std::process::id()));
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            source.to_str().expect("utf-8 path"),
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");

    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&output_path)
        .output()
        .expect("run compiled binary");
    assert_eq!(run.status.code(), Some(42));
}

#[test]
fn mo_build_compiles_and_runs_std_int_checked_arithmetic_and_width_guards() {
    let source = std::env::temp_dir().join(format!("mo_std_int_checked_{}.mo", std::process::id()));
    std::fs::write(
        &source,
        r#"
import * as int from "std/int"

fn main() -> Int {
    let add_ok = int.checked_add_or(40, 2, 0)
    let add_overflow = int.checked_add_or(int.max_value(), 1, 9)
    let sub_overflow = int.checked_sub_or(int.min_value(), 1, 8)
    let mul_overflow = int.checked_mul_or(int.max_value(), 2, 7)
    let parsed_overflow = int.parse_decimal_or("9223372036854775808", 6)

    if int.is_u8(255) {
        if int.to_u8_or(300, 5) == 5 {
            if int.to_i16_or(40000, 4) == 4 {
                return add_ok + add_overflow + sub_overflow + mul_overflow + parsed_overflow
            }
        }
    }
    return 1
}
"#,
    )
    .expect("write temp source");

    let output_path =
        std::env::temp_dir().join(format!("mo_std_int_checked_{}", std::process::id()));
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            source.to_str().expect("utf-8 path"),
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");

    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&output_path)
        .output()
        .expect("run compiled binary");
    assert_eq!(run.status.code(), Some(72));
}

#[test]
fn mo_build_compiles_and_runs_std_bytes_zero_copy_and_string_load() {
    let source = std::env::temp_dir().join(format!("mo_std_bytes_copy_{}.mo", std::process::id()));
    std::fs::write(
        &source,
        r#"
import * as bytes from "std/bytes"
import * as core from "core/unsafe"

fn main() -> Int {
    let src = core.alloc(3)
    let dst = core.alloc(3)
    bytes.store8(src, 0, bytes.string_load8("AZ", 0))
    bytes.store8(src, 1, bytes.string_load8("AZ", 1))
    bytes.store8(src, 2, 33)
    bytes.zero(dst, 3)
    bytes.copy(dst, src, 3)
    let result = bytes.load8(dst, 0) + bytes.load8(dst, 1) + bytes.load8(dst, 2)
    core.free(src)
    core.free(dst)
    return result
}
"#,
    )
    .expect("write temp source");

    let output_path =
        std::env::temp_dir().join(format!("mo_std_bytes_copy_{}", std::process::id()));
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            source.to_str().expect("utf-8 path"),
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");

    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&output_path)
        .output()
        .expect("run compiled binary");
    assert_eq!(run.status.code(), Some(188));
}

#[test]
fn mo_build_compiles_and_runs_narrow_integer_annotations() {
    let source = std::env::temp_dir().join(format!("mo_narrow_ints_{}.mo", std::process::id()));
    std::fs::write(
        &source,
        r#"
fn widen(value: UInt8) -> Int {
    return value
}

fn main() -> Int {
    let a: Int8 = -5
    let b: UInt8 = 40
    let c: Int16 = 3
    let d: UInt16 = 4
    return widen(b) + a + c + d
}
"#,
    )
    .expect("write temp source");

    let output_path = std::env::temp_dir().join(format!("mo_narrow_ints_{}", std::process::id()));
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            source.to_str().expect("utf-8 path"),
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");

    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&output_path)
        .output()
        .expect("run compiled binary");
    assert_eq!(run.status.code(), Some(42));
}

#[test]
fn mo_build_compiles_and_runs_narrow_integer_struct_fields() {
    let source = std::env::temp_dir().join(format!("mo_narrow_struct_{}.mo", std::process::id()));
    std::fs::write(
        &source,
        r#"
struct Packed {
    a: UInt8
    b: UInt16
    c: Int8
    d: Int32
}

fn main() -> Int {
    let packed = Packed { a: 250, b: 300, c: -8, d: 12 }
    return packed.a + packed.b + packed.c + packed.d
}
"#,
    )
    .expect("write temp source");

    let output_path = std::env::temp_dir().join(format!("mo_narrow_struct_{}", std::process::id()));
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            source.to_str().expect("utf-8 path"),
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");

    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&output_path)
        .output()
        .expect("run compiled binary");
    assert_eq!(run.status.code(), Some(42));
}

#[test]
fn mo_build_compiles_and_runs_narrow_integer_enum_payloads() {
    let source = std::env::temp_dir().join(format!("mo_narrow_enum_{}.mo", std::process::id()));
    std::fs::write(
        &source,
        r#"
enum Small {
    Byte(UInt8)
    Signed(Int8)
}

fn main() -> Int {
    let first: Small = Byte(250)
    let second: Small = Signed(-8)
    let a: Int = match first {
        Byte(value) => value
        Signed(value) => value
    }
    let b: Int = match second {
        Byte(value) => value
        Signed(value) => value
    }
    return a + b
}
"#,
    )
    .expect("write temp source");

    let output_path = std::env::temp_dir().join(format!("mo_narrow_enum_{}", std::process::id()));
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            source.to_str().expect("utf-8 path"),
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");

    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&output_path)
        .output()
        .expect("run compiled binary");
    assert_eq!(run.status.code(), Some(242));
}

#[test]
fn mo_build_compiles_and_runs_std_sync_atomic() {
    let code = build_and_run_exit_code("examples/compile/std_sync_atomic.mo", "mo_std_sync_atomic");
    assert_eq!(code, 42);
}

#[test]
fn mo_build_compiles_and_runs_channel_thread_int() {
    let code = build_and_run_exit_code(
        "examples/compile/channel_thread_int.mo",
        "mo_channel_thread_int",
    );
    assert_eq!(code, 42);
}

#[test]
fn mo_build_compiles_and_runs_generic_channel_int() {
    let code = build_and_run_exit_code(
        "examples/compile/generic_channel_int.mo",
        "mo_generic_channel_int",
    );
    assert_eq!(code, 42);
}

#[test]
fn mo_build_compiles_and_runs_generic_channel_string() {
    let code = build_and_run_exit_code(
        "examples/compile/generic_channel_string.mo",
        "mo_generic_channel_string",
    );
    assert_eq!(code, 42);
}

#[test]
fn mo_build_compiles_and_runs_inferred_channel_int() {
    let code = build_and_run_exit_code(
        "examples/compile/inferred_channel_int.mo",
        "mo_inferred_channel_int",
    );
    assert_eq!(code, 42);
}

#[test]
fn mo_build_compiles_and_runs_inferred_channel_bool() {
    let code = build_and_run_exit_code(
        "examples/compile/inferred_channel_bool.mo",
        "mo_inferred_channel_bool",
    );
    assert_eq!(code, 42);
}

#[test]
fn mo_build_compiles_and_runs_inferred_channel_string() {
    let code = build_and_run_exit_code(
        "examples/compile/inferred_channel_string.mo",
        "mo_inferred_channel_string",
    );
    assert_eq!(code, 42);
}

#[test]
fn mo_build_compiles_and_runs_channel_function_thread() {
    let code = build_and_run_exit_code(
        "examples/compile/channel_function_thread.mo",
        "mo_channel_function_thread",
    );
    assert_eq!(code, 42);
}

#[test]
fn mo_build_compiles_and_runs_channel_destroy_memory() {
    let code = build_and_run_exit_code(
        "examples/compile/channel_destroy_memory.mo",
        "mo_channel_destroy_memory",
    );
    assert_eq!(code, 42);
}

#[test]
fn mo_build_compiles_and_runs_box_int_auto_drop() {
    let code = build_and_run_exit_code(
        "examples/compile/box_int_auto_drop.mo",
        "mo_box_int_auto_drop",
    );
    assert_eq!(code, 42);
}

#[test]
fn mo_build_compiles_and_runs_box_int_take() {
    let code = build_and_run_exit_code("examples/compile/box_int_take.mo", "mo_box_int_take");
    assert_eq!(code, 42);
}

#[test]
fn mo_build_compiles_and_runs_box_string_auto_drop() {
    let code = build_and_run_exit_code(
        "examples/compile/box_string_auto_drop.mo",
        "mo_box_string_auto_drop",
    );
    assert_eq!(code, 42);
}

#[test]
fn mo_build_compiles_and_runs_box_string_take() {
    let code = build_and_run_exit_code("examples/compile/box_string_take.mo", "mo_box_string_take");
    assert_eq!(code, 42);
}

#[test]
fn mo_build_compiles_and_runs_drop_interface_auto_drop() {
    let code = build_and_run_exit_code(
        "examples/compile/drop_interface_auto_drop.mo",
        "mo_drop_interface_auto_drop",
    );
    assert_eq!(code, 42);
}

#[test]
fn mo_build_compiles_and_runs_vec_int_basic() {
    let code = build_and_run_exit_code("examples/compile/vec_int_basic.mo", "mo_vec_int_basic");
    assert_eq!(code, 42);
}

#[test]
fn mo_build_compiles_and_runs_vec_string_drop() {
    let code = build_and_run_exit_code("examples/compile/vec_string_drop.mo", "mo_vec_string_drop");
    assert_eq!(code, 42);
}

#[test]
fn mo_build_compiles_and_runs_vec_string_get() {
    let code = build_and_run_exit_code("examples/compile/vec_string_get.mo", "mo_vec_string_get");
    assert_eq!(code, 42);
}

#[test]
fn mo_build_compiles_and_runs_vec_handler_basic() {
    let code = build_and_run_exit_code(
        "examples/compile/vec_handler_basic.mo",
        "mo_vec_handler_basic",
    );
    assert_eq!(code, 42);
}

#[test]
fn mo_build_compiles_and_runs_map_string_string() {
    let code = build_and_run_exit_code(
        "examples/compile/map_string_string.mo",
        "mo_map_string_string",
    );
    assert_eq!(code, 42);
}

#[test]
fn mo_build_compiles_and_runs_std_http_read_request_headers() {
    let code = build_and_run_exit_code(
        "examples/compile/std_http_read_request_headers.mo",
        "mo_std_http_read_request_headers",
    );
    assert_eq!(code, 42);
}

#[test]
fn mo_build_compiles_and_runs_std_http_response_headers() {
    let code = build_and_run_exit_code(
        "examples/compile/std_http_response_headers.mo",
        "mo_std_http_response_headers",
    );
    assert_eq!(code, 42);
}

#[test]
fn mo_build_compiles_and_runs_std_http_read_request_content_length() {
    let code = build_and_run_exit_code(
        "examples/compile/std_http_read_request_content_length.mo",
        "mo_std_http_read_request_content_length",
    );
    assert_eq!(code, 42);
}

#[test]
fn mo_build_compiles_and_runs_std_http_read_request_line() {
    let code = build_and_run_exit_code(
        "examples/compile/std_http_read_request_line.mo",
        "mo_std_http_read_request_line",
    );
    assert_eq!(code, 42);
}

#[test]
fn mo_build_compiles_and_runs_shared_int_refcount() {
    let code = build_and_run_exit_code(
        "examples/compile/shared_int_refcount.mo",
        "mo_shared_int_refcount",
    );
    assert_eq!(code, 42);
}

#[test]
fn mo_build_compiles_and_runs_buffer_grow() {
    let code = build_and_run_exit_code("examples/compile/buffer_grow.mo", "mo_buffer_grow");
    assert_eq!(code, 42);
}

#[test]
fn mo_build_compiles_and_runs_buffer_grow_memory() {
    let code = build_and_run_exit_code(
        "examples/compile/buffer_grow_memory.mo",
        "mo_buffer_grow_memory",
    );
    assert_eq!(code, 42);
}

#[test]
fn mo_build_compiles_and_runs_string_builder_memory() {
    let code = build_and_run_exit_code(
        "examples/compile/string_builder_memory.mo",
        "mo_string_builder",
    );
    assert_eq!(code, 42);
}

#[test]
fn mo_build_compiles_and_runs_string_builder_auto_drop() {
    let code = build_and_run_exit_code(
        "examples/compile/string_builder_auto_drop.mo",
        "mo_string_builder_auto_drop",
    );
    assert_eq!(code, 42);
}

#[test]
fn mo_build_compiles_and_runs_byte_buffer_memory() {
    let code = build_and_run_exit_code("examples/compile/byte_buffer_memory.mo", "mo_byte_buffer");
    assert_eq!(code, 42);
}

#[test]
fn mo_build_compiles_and_runs_byte_buffer_auto_drop() {
    let code = build_and_run_exit_code(
        "examples/compile/byte_buffer_auto_drop.mo",
        "mo_byte_buffer_auto_drop",
    );
    assert_eq!(code, 42);
}

#[test]
fn mo_build_compiles_and_runs_slice_byte_view() {
    let code = build_and_run_exit_code("examples/compile/slice_byte_view.mo", "mo_slice_byte_view");
    assert_eq!(code, 42);
}

#[test]
fn mo_build_compiles_and_runs_slice_borrowed_memory() {
    let code = build_and_run_exit_code(
        "examples/compile/slice_borrowed_memory.mo",
        "mo_slice_borrowed_memory",
    );
    assert_eq!(code, 42);
}

#[test]
fn mo_build_compiles_and_runs_branch_early_return_drop_memory() {
    let code = build_and_run_exit_code(
        "examples/compile/branch_early_return_drop_memory.mo",
        "mo_branch_early_return_drop_memory",
    );
    assert_eq!(code, 42);
}

#[test]
fn mo_build_compiles_and_runs_string_reassign_drop_memory() {
    let code = build_and_run_exit_code(
        "examples/compile/string_reassign_drop_memory.mo",
        "mo_string_reassign_drop_memory",
    );
    assert_eq!(code, 42);
}

#[test]
fn mo_build_compiles_and_runs_nested_block_return_drop_memory() {
    let code = build_and_run_exit_code(
        "examples/compile/nested_block_return_drop_memory.mo",
        "mo_nested_block_return_drop_memory",
    );
    assert_eq!(code, 42);
}

#[test]
fn mo_build_compiles_and_runs_return_if_drop_memory() {
    let code = build_and_run_exit_code(
        "examples/compile/return_if_drop_memory.mo",
        "mo_return_if_drop_memory",
    );
    assert_eq!(code, 42);
}

#[test]
fn mo_build_compiles_and_runs_if_branch_fallthrough_drop_memory() {
    let code = build_and_run_exit_code(
        "examples/compile/if_branch_fallthrough_drop_memory.mo",
        "mo_if_branch_fallthrough_drop_memory",
    );
    assert_eq!(code, 42);
}

#[test]
fn mo_build_compiles_and_runs_loop_exit_drop_memory() {
    let code = build_and_run_exit_code(
        "examples/compile/loop_exit_drop_memory.mo",
        "mo_loop_exit_drop_memory",
    );
    assert_eq!(code, 42);
}

#[test]
fn mo_build_compiles_and_runs_buffer_auto_drop() {
    let code = build_and_run_exit_code(
        "examples/compile/buffer_auto_drop.mo",
        "mo_buffer_auto_drop",
    );
    assert_eq!(code, 42);
}

#[test]
fn mo_build_compiles_and_runs_task_threadpool4_atomic() {
    let code = build_and_run_exit_code(
        "examples/compile/task_threadpool4_atomic.mo",
        "mo_task_threadpool4_atomic",
    );
    assert_eq!(code, 42);
}

#[test]
fn mo_build_compiles_and_runs_task_queue4_int_jobs() {
    let code = build_and_run_exit_code(
        "examples/compile/task_queue4_int_jobs.mo",
        "mo_task_queue4_int_jobs",
    );
    assert_eq!(code, 42);
}

#[test]
fn mo_build_compiles_and_runs_task_queue4_int_close_safety() {
    let code = build_and_run_exit_code(
        "examples/compile/task_queue4_int_close_safety.mo",
        "mo_task_queue4_int_close_safety",
    );
    assert_eq!(code, 42);
}

#[test]
fn mo_build_compiles_and_runs_task_queue4_int_memory() {
    let code = build_and_run_exit_code(
        "examples/compile/task_queue4_int_memory.mo",
        "mo_task_queue4_int_memory",
    );
    assert_eq!(code, 42);
}

#[test]
fn mo_build_compiles_and_runs_task_queue4_int_auto_drop() {
    let code = build_and_run_exit_code(
        "examples/compile/task_queue4_int_auto_drop.mo",
        "mo_task_queue4_int_auto_drop",
    );
    assert_eq!(code, 42);
}

#[test]
fn mo_build_compiles_and_runs_bytecode_vm_demo() {
    let code = build_and_run_exit_code("demos/bytecode_vm/main.mo", "mo_bytecode_vm_demo");
    assert_eq!(code, 42);
}

#[test]
fn mo_build_compiles_and_runs_log_pipeline_demo() {
    let code = build_and_run_exit_code("demos/log_pipeline/main.mo", "mo_log_pipeline_demo");
    assert_eq!(code, 42);
}

#[test]
fn mo_build_compiles_and_runs_raytracer_demo() {
    let code = build_and_run_exit_code("demos/raytracer/main.mo", "mo_raytracer_demo");
    assert_eq!(code, 42);
}

#[test]
fn mo_build_compiles_and_runs_task_queue4_more_jobs() {
    let output_path =
        std::env::temp_dir().join(format!("mo_task_queue4_more_jobs_{}", std::process::id()));
    let output = Command::new(env!("CARGO_BIN_EXE_mo"))
        .args([
            "build",
            "examples/compile/task_queue4_more_jobs.mo",
            "-o",
            output_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run mo build");

    assert!(
        output.status.success(),
        "mo build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let run = Command::new(&output_path)
        .output()
        .expect("run compiled binary");
    assert_eq!(run.status.code(), Some(42));
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "job\njob\njob\njob\njob\njob\n"
    );
}
