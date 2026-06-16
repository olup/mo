use mo::hir::lower_program;
use mo::resolve::resolve_program;
use mo::semantics::{check_program, Target};
use mo::typeck::type_check_program;
use mo::{Lexer, Parser};

fn type_check(source: &str) -> Result<mo::typeck::TypeReport, Vec<mo::semantics::Diagnostic>> {
    let tokens = Lexer::new(source).tokenize().expect("lex");
    let program = Parser::new(tokens).parse_program().expect("parse");
    let target = Target::macos_aarch64();
    check_program(&program, &target)?;
    let hir = lower_program(&program, &target)?;
    resolve_program(&hir)?;
    type_check_program(&hir)
}

#[test]
fn accepts_simple_int_return_and_call() {
    let report = type_check(
        r#"
fn add(a: Int, b: Int) -> Int {
    return a + b
}

fn main() -> Int {
    return add(1, 2)
}
"#,
    )
    .expect("type check");

    assert!(report.checked_functions >= 2);
}

#[test]
fn reports_return_type_mismatch() {
    let errors = type_check(
        r#"
fn bad() -> Int {
    return "no"
}
"#,
    )
    .expect_err("expected return mismatch");

    assert!(errors
        .iter()
        .any(|error| error.message.contains("return type mismatch")));
}

#[test]
fn accepts_string_literal_as_str() {
    type_check(
        r#"
extern "C" {
    fn raw_string_concat(a: Str, b: Str) -> String
}

fn read(value: Str) -> Int {
    return 42
}

fn main() -> Int {
    return read("Ada")
}
"#,
    )
    .expect("string literal should type as borrowed Str");
}

#[test]
fn rejects_string_literal_as_owned_string_without_conversion() {
    let errors = type_check(
        r#"
fn consume(value: String) -> Int {
    return 1
}

fn main() -> Int {
    return consume("Ada")
}
"#,
    )
    .expect_err("string literal should not satisfy owned String without conversion");

    assert!(errors.iter().any(|error| {
        error
            .message
            .contains("argument 1 type mismatch in call to `consume`: expected String, got Str")
    }));
}

#[test]
fn accepts_owned_string_as_str_argument() {
    type_check(
        r#"
extern "C" {
    fn raw_string_concat(a: Str, b: Str) -> String
}

fn read(value: Str) -> Int {
    return 42
}

fn main() -> Int {
    let owned: String = raw_string_concat("Ada", "")
    return read(owned)
}
"#,
    )
    .expect("owned String should be readable through Str parameter");
}

#[test]
fn accepts_borrowed_str_reference_parameter_from_literal_and_owned_string() {
    type_check(
        r#"
extern "C" {
    fn raw_string_concat(a: Str, b: Str) -> String
}

fn read(value: &Str) -> Int {
    return 42
}

fn main() -> Int {
    let owned: String = raw_string_concat("Ada", "")
    return read("Grace") + read(owned)
}
"#,
    )
    .expect("read-only String/Str APIs should accept borrowed text views");
}

#[test]
fn accepts_byte_slice_index_expression() {
    type_check(
        r#"
struct ByteSlice {
    data: Str
    start: Int
    length_value: Int
}

fn main() -> Int {
    let view = ByteSlice { data: "abc", start: 0, length_value: 3 }
    return view[1]
}
"#,
    )
    .expect("ByteSlice indexing should type check");
}

#[test]
fn rejects_indexing_non_slice_value() {
    let errors = type_check(
        r#"
fn main() -> Int {
    let value = 1
    return value[0]
}
"#,
    )
    .expect_err("indexing a non-slice should fail");

    assert!(errors.iter().any(|error| error
        .message
        .contains("indexing is not supported for `Int`")));
}

#[test]
fn reports_wrong_call_arity() {
    let errors = type_check(
        r#"
fn add(a: Int, b: Int) -> Int {
    a + b
}

fn main() -> Int {
    add(1)
}
"#,
    )
    .expect_err("expected arity error");

    assert!(errors
        .iter()
        .any(|error| error.message.contains("expects 2 argument(s), got 1")));
}

#[test]
fn accepts_typed_raw_pointer_core_bridge() {
    type_check(
        r#"
fn raw_alloc(size: Int) -> Int {
    return 0
}

fn raw_load8(ptr: Int, offset: Int) -> Int {
    return 0
}

fn raw_store8(ptr: Int, offset: Int, value: Int) {
}

fn raw_free(ptr: Int) {
}

fn alloc_ptr(size: Int) -> *mut Byte {
    return raw_alloc(size)
}

fn load_ptr(ptr: *const Byte) -> Int {
    return raw_load8(ptr, 0)
}

fn main() -> Int {
    let ptr: *mut Byte = alloc_ptr(1)
    raw_store8(ptr, 0, 42)
    let value = load_ptr(ptr)
    raw_free(ptr)
    return value
}
"#,
    )
    .expect("typed raw pointer bridge should type check");
}

#[test]
fn rejects_raw_pointer_as_plain_byte_value() {
    let errors = type_check(
        r#"
fn raw_alloc(size: Int) -> Int {
    return 0
}

fn alloc_ptr(size: Int) -> *mut Byte {
    return raw_alloc(size)
}

fn main() {
    let ptr: *mut Byte = alloc_ptr(1)
    let byte: Byte = ptr
}
"#,
    )
    .expect_err("expected raw pointer to remain distinct from Byte");

    assert!(errors
        .iter()
        .any(|error| error.message.contains("let type mismatch")));
}

#[test]
fn accepts_named_function_as_callback_value() {
    type_check(
        r#"
fn inc(value: Int) -> Int {
    return value + 1
}

fn apply(callback: fn(Int) -> Int, value: Int) -> Int {
    return value
}

fn main() -> Int {
    return apply(inc, 41)
}
"#,
    )
    .expect("type check");
}

#[test]
fn accepts_non_capturing_closure_as_callback_value() {
    type_check(
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
    .expect("type check");
}

#[test]
fn reports_named_function_callback_signature_mismatch() {
    let errors = type_check(
        r#"
fn greet(value: String) -> String {
    return value
}

fn apply(callback: fn(Int) -> Int, value: Int) -> Int {
    return value
}

fn main() -> Int {
    return apply(greet, 41)
}
"#,
    )
    .expect_err("expected callback signature mismatch");

    assert!(errors.iter().any(|error| error.message.contains(
        "argument 1 type mismatch in call to `apply`: expected fn(Int) -> Int, got fn(String) -> String"
    )));
}

#[test]
fn accepts_integer_literals_that_fit_declared_widths() {
    type_check(
        r#"
fn takes_byte(value: UInt8) -> Int {
    return value
}

fn main() -> Int {
    let a: Int8 = -128
    let b: UInt8 = 255
    let c: Int16 = 32767
    let d: UInt16 = 65535
    return takes_byte(b)
}
"#,
    )
    .expect("narrow integer literals should type check");
}

#[test]
fn rejects_integer_literals_outside_declared_widths() {
    let errors = type_check(
        r#"
fn main() -> Int {
    let a: UInt8 = 256
    let b: Int8 = -129
    return 0
}
"#,
    )
    .expect_err("expected narrow integer range errors");

    assert!(errors.iter().any(|error| error
        .message
        .contains("integer literal 256 does not fit UInt8")));
    assert!(errors.iter().any(|error| error
        .message
        .contains("integer literal -129 does not fit Int8")));
}

#[test]
fn rejects_plain_int_variable_assigned_to_narrow_integer() {
    let errors = type_check(
        r#"
fn main() -> Int {
    let value = 42
    let byte: UInt8 = value
    return byte
}
"#,
    )
    .expect_err("expected explicit conversion requirement");

    assert!(errors
        .iter()
        .any(|error| error.message.contains("let type mismatch")));
}

#[test]
fn checks_struct_fields() {
    type_check(
        r#"
struct User {
    name: String
}

fn name(user: User) -> String {
    return user.name
}
"#,
    )
    .expect("type check");
}

#[test]
fn reports_unknown_struct_field() {
    let errors = type_check(
        r#"
struct User {
    name: String
}

fn bad(user: User) -> String {
    return user.missing
}
"#,
    )
    .expect_err("expected unknown field");

    assert!(errors
        .iter()
        .any(|error| error.message.contains("unknown field `missing` on `User`")));
}

#[test]
fn checks_generic_struct_field_substitution() {
    type_check(
        r#"
struct Holder<T> {
    value: T
}

extern "C" {
    fn raw_string_concat(a: Str, b: Str) -> String
}

fn main() -> String {
    let holder: Holder<String> = Holder { value: raw_string_concat("held", "") }
    return holder.value
}
"#,
    )
    .expect("generic struct field should substitute from annotation");
}

#[test]
fn infers_generic_struct_field_type_from_literal() {
    type_check(
        r#"
struct Holder<T> {
    value: T
}

extern "C" {
    fn raw_string_concat(a: Str, b: Str) -> String
}

fn main() -> String {
    let holder = Holder { value: raw_string_concat("held", "") }
    return holder.value
}
"#,
    )
    .expect("generic struct field should infer from literal value");
}

#[test]
fn rejects_generic_struct_field_substitution_mismatch() {
    let errors = type_check(
        r#"
struct Holder<T> {
    value: T
}

extern "C" {
    fn raw_string_concat(a: Str, b: Str) -> String
}

fn main() -> Int {
    let holder: Holder<Int> = Holder { value: raw_string_concat("bad", "") }
    return 0
}
"#,
    )
    .expect_err("expected generic struct field mismatch");

    assert!(errors.iter().any(|error| error
        .message
        .contains("struct field type mismatch: expected Int, got String")));
}

#[test]
fn checks_enum_payload_constructor_and_match_binding_types() {
    type_check(
        r#"
enum Value {
    Count(Int)
    Missing
}

fn main() -> Int {
    let value: Value = Count(41)
    return match value {
        Count(n) => n + 1
        Missing => 0
    }
}
"#,
    )
    .expect("enum payload should type check and bind payload type");
}

#[test]
fn rejects_enum_payload_constructor_type_mismatch() {
    let errors = type_check(
        r#"
enum Value {
    Count(Int)
    Missing
}

fn main() -> Int {
    let value: Value = Count("bad")
    return 0
}
"#,
    )
    .expect_err("expected enum payload type mismatch");

    assert!(errors.iter().any(|error| error
        .message
        .contains("argument 1 type mismatch in enum variant `Count`: expected Int, got Str")));
}

#[test]
fn rejects_enum_payload_constructor_arity_mismatch() {
    let errors = type_check(
        r#"
enum Value {
    Count(Int)
    Missing
}

fn main() -> Int {
    let value: Value = Count()
    return 0
}
"#,
    )
    .expect_err("expected enum payload arity mismatch");

    assert!(errors.iter().any(|error| error
        .message
        .contains("enum variant `Count` expects 1 argument(s), got 0")));
}

#[test]
fn rejects_generic_enum_payload_constructor_type_mismatch_from_annotation() {
    let errors = type_check(
        r#"
enum Option<T> {
    Some(T)
    None
}

fn main() -> Int {
    let value: Option<Int> = Some("bad")
    return 0
}
"#,
    )
    .expect_err("expected generic enum payload type mismatch");

    assert!(errors.iter().any(|error| error
        .message
        .contains("argument 1 type mismatch in enum variant `Some`: expected Int, got Str")));
}

#[test]
fn rejects_generic_enum_payload_constructor_type_mismatch_from_function_parameter() {
    let errors = type_check(
        r#"
enum Result<T, E> {
    Ok(T)
    Err(E)
}

fn consume(value: Result<Int, Str>) -> Int {
    return 0
}

fn main() -> Int {
    return consume(Ok("bad"))
}
"#,
    )
    .expect_err("expected generic enum payload type mismatch");

    assert!(errors.iter().any(|error| error
        .message
        .contains("argument 1 type mismatch in enum variant `Ok`: expected Int, got Str")));
}

#[test]
fn infers_generic_enum_payload_type_from_constructor_argument() {
    type_check(
        r#"
enum Option<T> {
    Some(T)
    None
}

fn main() -> Int {
    let value = Some(41)
    return match value {
        Some(item) => item + 1
        None => 0
    }
}
"#,
    )
    .expect("generic enum constructor payload should infer Option<Int>");
}

#[test]
fn inferred_generic_enum_payload_type_flows_to_match_binding() {
    let errors = type_check(
        r#"
enum Option<T> {
    Some(T)
    None
}

fn takes_str(value: Str) -> Int {
    return 0
}

fn main() -> Int {
    let value = Some(41)
    return match value {
        Some(item) => takes_str(item)
        None => 0
    }
}
"#,
    )
    .expect_err("expected match branch type mismatch from inferred generic payload");

    assert!(errors.iter().any(|error| error
        .message
        .contains("argument 1 type mismatch in call to `takes_str`: expected Str, got Int")));
}

#[test]
fn rejects_match_pattern_missing_payload_binding() {
    let errors = type_check(
        r#"
enum Option<T> {
    Some(T)
    None
}

fn main(value: Option<Int>) -> Int {
    return match value {
        Some() => 1
        None => 0
    }
}
"#,
    )
    .expect_err("expected match pattern payload arity mismatch");

    assert!(errors.iter().any(|error| error
        .message
        .contains("match pattern `Some` expects 1 binding(s), got 0")));
}

#[test]
fn rejects_match_pattern_extra_payload_binding() {
    let errors = type_check(
        r#"
enum Option<T> {
    Some(T)
    None
}

fn main(value: Option<Int>) -> Int {
    return match value {
        Some(item) => item
        None(item) => item
    }
}
"#,
    )
    .expect_err("expected match pattern payload arity mismatch");

    assert!(errors.iter().any(|error| error
        .message
        .contains("match pattern `None` expects 0 binding(s), got 1")));
}

#[test]
fn checks_result_try_unwrap_type_and_error_propagation() {
    type_check(
        r#"
enum Result<T, E> {
    Ok(T)
    Err(E)
}

fn parse() -> Result<Int, Str> {
    return Ok(41)
}

fn main() -> Result<Int, Str> {
    let value: Int = parse()?
    return Ok(value + 1)
}
"#,
    )
    .expect("result try should unwrap ok type and propagate matching error type");
}

#[test]
fn rejects_try_on_non_result_or_option() {
    let errors = type_check(
        r#"
fn main() -> Int {
    let value = 41?
    return value
}
"#,
    )
    .expect_err("expected try operand type mismatch");

    assert!(errors.iter().any(|error| error
        .message
        .contains("`?` requires Result<T, E> or Option<T>, got integer literal")));
}

#[test]
fn rejects_result_try_in_non_result_function() {
    let errors = type_check(
        r#"
enum Result<T, E> {
    Ok(T)
    Err(E)
}

fn parse() -> Result<Int, Str> {
    return Ok(41)
}

fn main() -> Int {
    let value = parse()?
    return value
}
"#,
    )
    .expect_err("expected result try return type mismatch");

    assert!(errors.iter().any(|error| error
        .message
        .contains("`?` on Result requires enclosing return type Result<_, _>, got Int")));
}

#[test]
fn rejects_result_try_error_type_mismatch() {
    let errors = type_check(
        r#"
enum Result<T, E> {
    Ok(T)
    Err(E)
}

fn parse() -> Result<Int, Int> {
    return Ok(41)
}

fn main() -> Result<Int, Str> {
    let value = parse()?
    return Ok(value)
}
"#,
    )
    .expect_err("expected result try error type mismatch");

    assert!(errors.iter().any(|error| error
        .message
        .contains("`?` error type mismatch: expected Str, got Int")));
}

#[test]
fn rejects_option_try_in_non_option_function() {
    let errors = type_check(
        r#"
enum Option<T> {
    Some(T)
    None
}

fn maybe() -> Option<Int> {
    return Some(41)
}

fn main() -> Int {
    let value = maybe()?
    return value
}
"#,
    )
    .expect_err("expected option try return type mismatch");

    assert!(errors.iter().any(|error| error
        .message
        .contains("`?` on Option requires enclosing return type Option<_>, got Int")));
}

#[test]
fn treats_returning_if_branch_as_never_for_join_type() {
    type_check(
        r#"
fn main(flag: Bool) -> Int {
    let value: Int = if flag {
        return 41
    } else {
        42
    }
    return value
}
"#,
    )
    .expect("returning if branch should not poison joined expression type");
}

#[test]
fn treats_returning_match_arm_as_never_for_join_type() {
    type_check(
        r#"
enum Option<T> {
    Some(T)
    None
}

fn main(value: Option<Int>) -> Int {
    let unwrapped: Int = match value {
        Some(item) => item
        None => if true {
            return 0
        } else {
            return 1
        }
    }
    return unwrapped
}
"#,
    )
    .expect("returning match arm should not poison joined expression type");
}

#[test]
fn still_checks_return_expression_inside_never_branch() {
    let errors = type_check(
        r#"
fn main(flag: Bool) -> Str {
    let value: Str = if flag {
        return 41
    } else {
        "ok"
    }
    return value
}
"#,
    )
    .expect_err("expected return type mismatch inside returning branch");

    assert!(errors.iter().any(|error| error
        .message
        .contains("return type mismatch: expected Str, got integer literal")));
}

#[test]
fn checks_explicit_generic_function_call_type_substitution() {
    type_check(
        r#"
fn identity<T>(value: T) -> T {
    return value
}

fn main() -> Int {
    return identity<Int>(41)
}
"#,
    )
    .expect("explicit generic function call should substitute parameter and return types");
}

#[test]
fn rejects_explicit_generic_function_call_argument_mismatch() {
    let errors = type_check(
        r#"
fn identity<T>(value: T) -> T {
    return value
}

fn main() -> Int {
    return identity<Int>("bad")
}
"#,
    )
    .expect_err("expected generic function argument mismatch");

    assert!(errors.iter().any(|error| error
        .message
        .contains("argument 1 type mismatch in call to `identity`: expected Int, got Str")));
}

#[test]
fn rejects_explicit_generic_function_type_argument_arity_mismatch() {
    let errors = type_check(
        r#"
fn pair<T, U>(left: T, right: U) -> T {
    return left
}

fn main() -> Int {
    return pair<Int>(1, 2)
}
"#,
    )
    .expect_err("expected generic function type argument arity mismatch");

    assert!(errors.iter().any(|error| error
        .message
        .contains("`pair` expects 2 type argument(s), got 1")));
}

#[test]
fn infers_generic_function_type_argument_from_expected_return() {
    type_check(
        r#"
fn identity<T>(value: T) -> T {
    return value
}

fn main() -> Int {
    return identity(41)
}
"#,
    )
    .expect("generic function call should infer type argument from return context");
}

#[test]
fn infers_generic_function_type_argument_from_parameter_only() {
    type_check(
        r#"
fn consume<T>(value: T) -> Int {
    return 42
}

fn main() -> Int {
    return consume("ok")
}
"#,
    )
    .expect("generic function call should infer type argument from argument type");
}

#[test]
fn rejects_conflicting_inferred_generic_function_type_arguments() {
    let errors = type_check(
        r#"
fn choose<T>(left: T, right: T) -> T {
    return left
}

fn main() -> Int {
    return choose(1, "bad")
}
"#,
    )
    .expect_err("expected conflicting generic function inference");

    assert!(errors.iter().any(|error| error
        .message
        .contains("conflicting inferred type for `T` in call to `choose`: expected Int, got Str")));
}

#[test]
fn resolves_method_call_by_receiver_first_parameter_type() {
    type_check(
        r#"
struct App {
    id: Int
}

fn express__get(app: &mut App, path: Str) -> Int {
    return app.id
}

fn main() -> Int {
    let mut app = App { id: 42 }
    return app.get("/pokemon")
}
"#,
    )
    .expect("method call should resolve through receiver-compatible first parameter");
}

#[test]
fn rejects_suffix_method_candidate_with_wrong_receiver_type() {
    let errors = type_check(
        r#"
struct App {
    id: Int
}

struct Other {
    id: Int
}

fn express__get(other: Other, path: Str) -> Int {
    return other.id
}

fn main() -> Int {
    let mut app = App { id: 42 }
    return app.get("/pokemon")
}
"#,
    )
    .expect_err("expected unknown method for wrong receiver type");

    assert!(errors
        .iter()
        .any(|error| error.message.contains("unknown method `get` for `App`")));
}

#[test]
fn reference_examples_type_check_skeleton() {
    for path in [
        "examples/reference/core.mo",
        "examples/reference/types.mo",
        "examples/reference/methods_interfaces.mo",
        "examples/reference/memory_errors.mo",
        "examples/reference/closures_async_threads.mo",
        "examples/reference/platform.mo",
        "examples/reference/web_server.mo",
    ] {
        let source = std::fs::read_to_string(path).expect(path);
        type_check(&source).unwrap_or_else(|errors| panic!("{path}: {errors:#?}"));
    }
}
