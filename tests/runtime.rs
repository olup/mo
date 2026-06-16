use mo::borrow::check_borrows;
use mo::dropck::check_drops;
use mo::hir::lower_program;
use mo::ir::lower_to_ir;
use mo::ownership::check_ownership;
use mo::resolve::resolve_program;
use mo::runtime::runtime_for_target;
use mo::semantics::{check_program, ObjectFormat, Target};
use mo::typeck::type_check_program;
use mo::{Lexer, Parser};

#[test]
fn macos_runtime_config_has_expected_target_and_symbols() {
    let config = runtime_for_target(&Target::macos_aarch64());

    assert_eq!(config.triple, "aarch64-apple-darwin");
    assert!(config.libc_symbols.contains(&"getpid".to_string()));
    assert!(config.libc_symbols.contains(&"strlen".to_string()));
    assert!(config.libc_symbols.contains(&"write".to_string()));
    assert!(config
        .thread_symbols
        .contains(&"pthread_create".to_string()));
    assert!(config.socket_symbols.contains(&"socket".to_string()));
    assert!(config
        .time_symbols
        .contains(&"mach_absolute_time".to_string()));
    assert!(config.allocator_symbols.contains(&"malloc".to_string()));
}

#[test]
fn linux_runtime_config_has_target_stubs_and_symbols() {
    let x86 = runtime_for_target(&Target::linux_x86_64());
    assert_eq!(x86.triple, "x86_64-unknown-linux-gnu");
    assert!(x86.libc_symbols.contains(&"getpid".to_string()));
    assert!(x86.thread_symbols.contains(&"pthread_create".to_string()));
    assert!(x86.socket_symbols.contains(&"socket".to_string()));
    assert!(x86.time_symbols.contains(&"clock_gettime".to_string()));
    assert!(x86.allocator_symbols.contains(&"malloc".to_string()));

    let arm = runtime_for_target(&Target::linux_aarch64());
    assert_eq!(arm.triple, "aarch64-unknown-linux-gnu");
}

#[test]
fn target_parse_accepts_aliases_and_reports_object_formats() {
    let linux = Target::parse("linux-x86_64").expect("linux target");
    assert_eq!(linux.triple(), "x86_64-unknown-linux-gnu");
    assert!(linux.has("linux"));
    assert!(linux.has("x86_64"));
    assert_eq!(linux.object_format(), ObjectFormat::Elf);

    let macos = Target::parse("aarch64-apple-darwin").expect("macos target");
    assert_eq!(macos.triple(), "aarch64-apple-darwin");
    assert!(macos.has("macos"));
    assert!(macos.has("darwin"));
    assert_eq!(macos.object_format(), ObjectFormat::MachO);

    assert!(Target::parse("plan9-riscv64").is_err());
}

#[test]
fn linux_target_checks_target_independent_code() {
    let source = r#"
fn answer() -> Int {
    return 42
}
"#;

    let tokens = Lexer::new(source).tokenize().expect("lex");
    let program = Parser::new(tokens).parse_program().expect("parse");
    let target = Target::linux_x86_64();
    check_program(&program, &target).expect("semantic");
    let hir = lower_program(&program, &target).expect("lower");
    resolve_program(&hir).expect("resolve");
    type_check_program(&hir).expect("type check");
}

#[test]
fn extern_declarations_lower_to_hir_and_ir() {
    let source = r#"
extern "C" {
    fn getpid() -> Int32
}

fn main() -> Int32 {
    return getpid()
}
"#;

    let tokens = Lexer::new(source).tokenize().expect("lex");
    let program = Parser::new(tokens).parse_program().expect("parse");
    let target = Target::macos_aarch64();
    check_program(&program, &target).expect("semantic");
    let hir = lower_program(&program, &target).expect("lower");
    resolve_program(&hir).expect("resolve");
    type_check_program(&hir).expect("type check");
    check_ownership(&hir).expect("ownership");
    check_borrows(&hir).expect("borrow");
    let drops = check_drops(&hir).expect("drops");
    let ir = lower_to_ir(&hir, &drops);

    assert_eq!(hir.extern_functions[0].name, "getpid");
    assert_eq!(hir.extern_functions[0].abi.as_deref(), Some("C"));
    assert_eq!(ir.extern_functions[0].name, "getpid");
    assert_eq!(ir.extern_functions[0].abi.as_deref(), Some("C"));
}
