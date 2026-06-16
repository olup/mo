use mo::borrow::check_borrows;
use mo::dropck::check_drops;
use mo::hir::lower_program;
use mo::ir::{
    lower_to_ir, IrBoolExpr, IrCompareOp, IrEnumExpr, IrFloatExpr, IrFunctionExpr, IrInstruction,
    IrIntExpr, IrStringExpr, IrStructExpr, IrTerminator, IrValueExpr,
};
use mo::ownership::check_ownership;
use mo::resolve::resolve_program;
use mo::semantics::{check_program, Target};
use mo::typeck::type_check_program;
use mo::{Lexer, Parser};

fn ir_for(source: &str) -> mo::ir::IrProgram {
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
    lower_to_ir(&hir, &drops)
}

fn int_expr_calls(expr: &IrIntExpr, callee_name: &str) -> bool {
    match expr {
        IrIntExpr::Call { callee, args } => {
            callee == callee_name || args.iter().any(|arg| value_expr_calls(arg, callee_name))
        }
        IrIntExpr::IndirectCall { callee, args } => {
            function_expr_calls(callee, callee_name)
                || args.iter().any(|arg| value_expr_calls(arg, callee_name))
        }
        IrIntExpr::StringLen(value) | IrIntExpr::StringPtr(value) => {
            string_expr_calls(value, callee_name)
        }
        IrIntExpr::EnumTag(value) => enum_expr_calls(value, callee_name),
        IrIntExpr::FunctionPtr(value) => function_expr_calls(value, callee_name),
        IrIntExpr::FloatToInt(value) => float_expr_calls(value, callee_name),
        IrIntExpr::RawWrite { fd, text } => {
            int_expr_calls(fd, callee_name) || string_expr_calls(text, callee_name)
        }
        IrIntExpr::RawAlloc { size } => int_expr_calls(size, callee_name),
        IrIntExpr::RawLoad8 { ptr, offset } | IrIntExpr::RawLoad64 { ptr, offset } => {
            int_expr_calls(ptr, callee_name) || int_expr_calls(offset, callee_name)
        }
        IrIntExpr::RawSetNonblocking { fd } => int_expr_calls(fd, callee_name),
        IrIntExpr::RawThreadSpawn { task, captures } => {
            function_expr_calls(task, callee_name)
                || captures
                    .iter()
                    .any(|capture| value_expr_calls(capture, callee_name))
        }
        IrIntExpr::RawThreadJoin { handle } => int_expr_calls(handle, callee_name),
        IrIntExpr::Binary { left, right, .. } => {
            int_expr_calls(left, callee_name) || int_expr_calls(right, callee_name)
        }
        IrIntExpr::Const(_)
        | IrIntExpr::Local(_)
        | IrIntExpr::Field { .. }
        | IrIntExpr::EnvLoad { .. }
        | IrIntExpr::RawMemAllocCount
        | IrIntExpr::RawMemFreeCount
        | IrIntExpr::RawMemLiveBytes
        | IrIntExpr::RawMemHighWaterBytes => false,
    }
}

fn bool_expr_calls(expr: &IrBoolExpr, callee_name: &str) -> bool {
    match expr {
        IrBoolExpr::Call { callee, args } => {
            callee == callee_name || args.iter().any(|arg| value_expr_calls(arg, callee_name))
        }
        IrBoolExpr::Not(value) => bool_expr_calls(value, callee_name),
        IrBoolExpr::And(left, right) | IrBoolExpr::Or(left, right) => {
            bool_expr_calls(left, callee_name) || bool_expr_calls(right, callee_name)
        }
        IrBoolExpr::Compare { left, right, .. } => {
            int_expr_calls(left, callee_name) || int_expr_calls(right, callee_name)
        }
        IrBoolExpr::FloatCompare { left, right, .. } => {
            float_expr_calls(left, callee_name) || float_expr_calls(right, callee_name)
        }
        IrBoolExpr::BoolCompare { left, right, .. } => {
            bool_expr_calls(left, callee_name) || bool_expr_calls(right, callee_name)
        }
        IrBoolExpr::StringCompare { left, right, .. } => {
            string_expr_calls(left, callee_name) || string_expr_calls(right, callee_name)
        }
        IrBoolExpr::Const(_)
        | IrBoolExpr::Local(_)
        | IrBoolExpr::Field { .. }
        | IrBoolExpr::EnvLoad { .. } => false,
    }
}

fn float_expr_calls(expr: &IrFloatExpr, callee_name: &str) -> bool {
    match expr {
        IrFloatExpr::Call { callee, args } => {
            callee == callee_name || args.iter().any(|arg| value_expr_calls(arg, callee_name))
        }
        IrFloatExpr::IndirectCall { callee, args } => {
            function_expr_calls(callee, callee_name)
                || args.iter().any(|arg| value_expr_calls(arg, callee_name))
        }
        IrFloatExpr::IntToFloat(value) => int_expr_calls(value, callee_name),
        IrFloatExpr::Binary { left, right, .. } => {
            float_expr_calls(left, callee_name) || float_expr_calls(right, callee_name)
        }
        IrFloatExpr::Const(_)
        | IrFloatExpr::Local(_)
        | IrFloatExpr::Field { .. }
        | IrFloatExpr::EnvLoad { .. } => false,
    }
}

fn string_expr_calls(expr: &IrStringExpr, callee_name: &str) -> bool {
    match expr {
        IrStringExpr::RawAlloc { size }
        | IrStringExpr::IntToString(size)
        | IrStringExpr::FromPtr(size) => int_expr_calls(size, callee_name),
        IrStringExpr::Concat { left, right } => {
            string_expr_calls(left, callee_name) || string_expr_calls(right, callee_name)
        }
        IrStringExpr::Call { callee, args } => {
            callee == callee_name || args.iter().any(|arg| value_expr_calls(arg, callee_name))
        }
        IrStringExpr::IndirectCall { callee, args } => {
            function_expr_calls(callee, callee_name)
                || args.iter().any(|arg| value_expr_calls(arg, callee_name))
        }
        IrStringExpr::Literal(_)
        | IrStringExpr::Local(_)
        | IrStringExpr::EnvLoad { .. }
        | IrStringExpr::Field { .. } => false,
    }
}

fn struct_expr_calls(expr: &IrStructExpr, callee_name: &str) -> bool {
    match expr {
        IrStructExpr::Construct { fields, .. } => fields
            .iter()
            .any(|field| value_expr_calls(&field.value, callee_name)),
        IrStructExpr::Call { callee, args } => {
            callee == callee_name || args.iter().any(|arg| value_expr_calls(arg, callee_name))
        }
        IrStructExpr::IndirectCall { callee, args } => {
            function_expr_calls(callee, callee_name)
                || args.iter().any(|arg| value_expr_calls(arg, callee_name))
        }
        IrStructExpr::Local(_) | IrStructExpr::Field { .. } | IrStructExpr::EnvLoad { .. } => false,
    }
}

fn enum_expr_calls(expr: &IrEnumExpr, callee_name: &str) -> bool {
    match expr {
        IrEnumExpr::Call { callee, args } => {
            callee == callee_name || args.iter().any(|arg| value_expr_calls(arg, callee_name))
        }
        IrEnumExpr::IndirectCall { callee, args } => {
            function_expr_calls(callee, callee_name)
                || args.iter().any(|arg| value_expr_calls(arg, callee_name))
        }
        IrEnumExpr::Construct { payloads, .. } => payloads
            .iter()
            .any(|payload| value_expr_calls(payload, callee_name)),
        IrEnumExpr::Local(_) | IrEnumExpr::EnvLoad { .. } => false,
    }
}

fn function_expr_calls(expr: &IrFunctionExpr, callee_name: &str) -> bool {
    match expr {
        IrFunctionExpr::Call { callee, args } => {
            callee == callee_name || args.iter().any(|arg| value_expr_calls(arg, callee_name))
        }
        IrFunctionExpr::FromPtr(value) => int_expr_calls(value, callee_name),
        IrFunctionExpr::Local(_) | IrFunctionExpr::Named(_) | IrFunctionExpr::Field { .. } => false,
    }
}

fn value_expr_calls(expr: &IrValueExpr, callee_name: &str) -> bool {
    match expr {
        IrValueExpr::Int(value) => int_expr_calls(value, callee_name),
        IrValueExpr::Float(value) => float_expr_calls(value, callee_name),
        IrValueExpr::Bool(value) => bool_expr_calls(value, callee_name),
        IrValueExpr::String(value) => string_expr_calls(value, callee_name),
        IrValueExpr::Struct(value) => struct_expr_calls(value, callee_name),
        IrValueExpr::Enum(value) => enum_expr_calls(value, callee_name),
        IrValueExpr::Function(value) => function_expr_calls(value, callee_name),
    }
}

fn instruction_calls(instruction: &IrInstruction, callee_name: &str) -> bool {
    match instruction {
        IrInstruction::AssignInt { value, .. }
        | IrInstruction::ReturnInt { value }
        | IrInstruction::PrintInt { value } => int_expr_calls(value, callee_name),
        IrInstruction::AssignFloat { value, .. } | IrInstruction::ReturnFloat { value } => {
            float_expr_calls(value, callee_name)
        }
        IrInstruction::AssignBool { value, .. }
        | IrInstruction::ReturnBool { value }
        | IrInstruction::Assert {
            condition: value, ..
        } => bool_expr_calls(value, callee_name),
        IrInstruction::AssignString { value, .. }
        | IrInstruction::ReturnString { value }
        | IrInstruction::PrintStringExpr { value } => string_expr_calls(value, callee_name),
        IrInstruction::AssignStruct { value, .. } | IrInstruction::ReturnStruct { value } => {
            struct_expr_calls(value, callee_name)
        }
        IrInstruction::AssignEnum { value, .. } | IrInstruction::ReturnEnum { value } => {
            enum_expr_calls(value, callee_name)
        }
        IrInstruction::AssignFunction { value, .. } | IrInstruction::ReturnFunction { value } => {
            function_expr_calls(value, callee_name)
        }
        IrInstruction::AssignField { value, .. } => value_expr_calls(value, callee_name),
        IrInstruction::AssignEnumMatch { value, arms, .. } => {
            enum_expr_calls(value, callee_name)
                || arms
                    .iter()
                    .any(|arm| value_expr_calls(&arm.body, callee_name))
        }
        IrInstruction::BindEnumPayload { value, .. } => enum_expr_calls(value, callee_name),
        IrInstruction::ReturnEnumMatch { value, arms, .. } => {
            enum_expr_calls(value, callee_name)
                || arms
                    .iter()
                    .any(|arm| value_expr_calls(&arm.body, callee_name))
        }
        IrInstruction::RawWrite { fd, text } => {
            int_expr_calls(fd, callee_name) || string_expr_calls(text, callee_name)
        }
        IrInstruction::RawStore8 { ptr, offset, value }
        | IrInstruction::RawStore64 { ptr, offset, value } => {
            int_expr_calls(ptr, callee_name)
                || int_expr_calls(offset, callee_name)
                || int_expr_calls(value, callee_name)
        }
        IrInstruction::RawStringStore8 {
            value,
            offset,
            byte,
        } => {
            string_expr_calls(value, callee_name)
                || int_expr_calls(offset, callee_name)
                || int_expr_calls(byte, callee_name)
        }
        IrInstruction::RawFree { ptr } => int_expr_calls(ptr, callee_name),
        IrInstruction::Call { callee, args } => {
            callee == callee_name || args.iter().any(|arg| value_expr_calls(arg, callee_name))
        }
        IrInstruction::IndirectCall { callee, args } => {
            function_expr_calls(callee, callee_name)
                || args.iter().any(|arg| value_expr_calls(arg, callee_name))
        }
        IrInstruction::Let { .. }
        | IrInstruction::ConstInt { .. }
        | IrInstruction::ReturnUnit
        | IrInstruction::PrintString { .. }
        | IrInstruction::DropBoxStorage { .. }
        | IrInstruction::Expr { .. }
        | IrInstruction::Borrow { .. }
        | IrInstruction::Suspend { .. }
        | IrInstruction::FutureState { .. }
        | IrInstruction::BorrowAcrossSuspend { .. }
        | IrInstruction::Try { .. }
        | IrInstruction::Drop { .. } => false,
    }
}

fn terminator_calls(terminator: &IrTerminator, callee_name: &str) -> bool {
    match terminator {
        IrTerminator::Branch {
            condition: Some(condition),
            ..
        } => bool_expr_calls(condition, callee_name),
        IrTerminator::Switch { value, .. } => enum_expr_calls(value, callee_name),
        IrTerminator::Return
        | IrTerminator::Jump { .. }
        | IrTerminator::Branch {
            condition: None, ..
        }
        | IrTerminator::Unreachable => false,
    }
}

#[test]
fn simple_function_lowers_to_ir() {
    let ir = ir_for(
        r#"
fn add(a: Int, b: Int) -> Int {
    return a + b
}
"#,
    );

    let add = ir.function("add").expect("add function");
    assert_eq!(add.params, vec!["a".to_string(), "b".to_string()]);
    assert_eq!(add.blocks.len(), 1);
    assert!(matches!(add.blocks[0].terminator, IrTerminator::Return));
}

#[test]
fn named_function_callback_lowers_to_indirect_call() {
    let ir = ir_for(
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
    );

    let apply = ir.function("apply").expect("apply function");
    assert!(matches!(
        apply.blocks[0].instructions.last(),
        Some(IrInstruction::ReturnInt {
            value: IrIntExpr::IndirectCall {
                callee: IrFunctionExpr::Local(name),
                args,
            },
        }) if name == "callback" && args.len() == 1
    ));
    let main = ir.function("main").expect("main function");
    assert!(matches!(
        main.blocks[0].instructions.last(),
        Some(IrInstruction::ReturnInt {
            value: IrIntExpr::Call { callee, args },
        }) if callee == "apply" && args.len() == 2
    ));
}

#[test]
fn enum_returning_callback_lowers_to_enum_indirect_call() {
    let ir = ir_for(
        r#"
enum Option<T> {
    Some(T)
    None
}

fn keep_positive(value: Int) -> Option<Int> {
    if value > 0 {
        return Some(value + 1)
    }
    return None
}

fn and_then(value: Option<Int>, mapper: fn(Int) -> Option<Int>) -> Option<Int> {
    return match value {
        Some(item) => mapper(item)
        None => None
    }
}
"#,
    );

    let keep_positive = ir
        .function("keep_positive")
        .expect("keep_positive function");
    assert!(keep_positive.blocks.iter().any(|block| {
        matches!(
            block.instructions.last(),
            Some(IrInstruction::ReturnEnum {
                value: IrEnumExpr::Construct { variant, .. },
            }) if variant == "Some"
        )
    }));
    assert!(keep_positive.blocks.iter().any(|block| {
        matches!(
            block.instructions.last(),
            Some(IrInstruction::ReturnEnum {
                value: IrEnumExpr::Construct { variant, .. },
            }) if variant == "None"
        )
    }));

    let and_then = ir.function("and_then").expect("and_then function");
    assert!(matches!(
        and_then.blocks[0].instructions.last(),
        Some(IrInstruction::ReturnEnumMatch { arms, .. })
            if arms.iter().any(|arm| matches!(
                &arm.body,
                mo::ir::IrValueExpr::Enum(IrEnumExpr::IndirectCall {
                    callee: IrFunctionExpr::Local(name),
                    args,
                }) if name == "mapper" && args.len() == 1
            ))
    ));
}

#[test]
fn hinted_enum_constructor_lowers_generic_callback_payload() {
    let ir = ir_for(
        r#"
enum Result<T, E> {
    Ok(T)
    Err(E)
}

fn map_err(value: Result<Int, Int>, mapper: fn(Int) -> Int) -> Result<Int, Int> {
    return match value {
        Ok(item) => Ok(item)
        Err(error) => Err(mapper(error))
    }
}
"#,
    );

    let map_err = ir.function("map_err").expect("map_err function");
    assert!(matches!(
        map_err.blocks[0].instructions.last(),
        Some(IrInstruction::ReturnEnumMatch { arms, .. })
            if arms.iter().any(|arm| matches!(
                &arm.body,
                mo::ir::IrValueExpr::Enum(IrEnumExpr::Construct {
                    variant,
                    payloads,
                    ..
                }) if variant == "Err"
                    && matches!(
                        payloads.first(),
                        Some(mo::ir::IrValueExpr::Int(IrIntExpr::IndirectCall {
                            callee: IrFunctionExpr::Local(name),
                            args,
                        })) if name == "mapper" && args.len() == 1
                    )
            ))
    ));
}

#[test]
fn non_capturing_closure_callback_lowers_to_generated_function() {
    let ir = ir_for(
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
    );

    let generated = ir
        .functions
        .iter()
        .find(|function| function.name.starts_with("__closure_main_"))
        .expect("generated closure function");
    assert!(matches!(
        generated.blocks[0].instructions.last(),
        Some(IrInstruction::ReturnInt { .. })
    ));
    let main = ir.function("main").expect("main function");
    assert!(main.blocks[0].instructions.iter().any(|instruction| {
        matches!(
            instruction,
            IrInstruction::AssignFunction {
                local,
                value: IrFunctionExpr::Named(name),
            } if local == "inc" && name.starts_with("__closure_main_")
        )
    }));
    assert!(matches!(
        main.blocks[0].instructions.last(),
        Some(IrInstruction::ReturnInt {
            value: IrIntExpr::Call { callee, args },
        }) if callee == "apply" && args.len() == 2
    ));
}

#[test]
fn string_return_annotation_lowers_as_owned_boundary() {
    let ir = ir_for(
        r#"
extern "C" {
    fn raw_string_concat(a: Str, b: Str) -> String
}

fn main() -> String {
    return raw_string_concat("Ada", "")
}
"#,
    );

    let main = ir.function("main").expect("main function");
    assert_eq!(main.return_type, mo::ir::IrValueTy::OwnedString);
}

#[test]
fn closure_string_return_annotation_lowers_as_owned_boundary() {
    let ir = ir_for(
        r#"
extern "C" {
    fn raw_string_concat(a: Str, b: Str) -> String
}

fn apply(callback: fn() -> String) -> String {
    return callback()
}

fn main() -> String {
    let make = fn() -> String {
        return raw_string_concat("Ada", "")
    }
    return apply(make)
}
"#,
    );

    let generated = ir
        .functions
        .iter()
        .find(|function| function.name.starts_with("__closure_main_"))
        .expect("generated closure function");
    assert_eq!(generated.return_type, mo::ir::IrValueTy::OwnedString);

    let apply = ir.function("apply").expect("apply function");
    assert!(matches!(
        apply.blocks[0].instructions.last(),
        Some(IrInstruction::ReturnString {
            value: IrStringExpr::IndirectCall {
                callee: IrFunctionExpr::Local(name),
                args,
            },
        }) if name == "callback" && args.is_empty()
    ));
}

#[test]
fn raw_int_to_string_lowers_to_intrinsic_string_expr() {
    let ir = ir_for(
        r#"
fn core__int_to_string(value: Int) -> String {
    return raw_int_to_string(value)
}

fn main() -> String {
    return core__int_to_string(42)
}
"#,
    );

    let main = ir
        .function("core__int_to_string")
        .expect("core wrapper function");
    assert!(matches!(
        main.blocks[0].instructions.last(),
        Some(IrInstruction::ReturnString {
            value: mo::ir::IrStringExpr::IntToString(_),
        })
    ));
}

#[test]
fn ir_preserves_function_module_id() {
    let ir = ir_for(
        r#"
module app.main

fn main() -> Int {
    return 0
}
"#,
    );

    let module = ir
        .modules
        .iter()
        .find(|module| module.path == vec!["app".to_string(), "main".to_string()])
        .expect("app.main module");
    let main = ir.function("main").expect("main function");
    assert_eq!(main.module, module.id);
}

#[test]
fn ir_builds_module_qualified_backend_symbols() {
    let ir = ir_for(
        r#"
module app.math

fn add(a: Int, b: Int) -> Int {
    return a + b
}

fn main() -> Int {
    return add(20, 22)
}
"#,
    );

    let add = ir.function("add").expect("add function");
    let main = ir.function("main").expect("main function");
    assert_eq!(ir.function_symbol(add), "__mo_app__math__add");
    assert_eq!(ir.function_symbol(main), "main");
}

#[test]
fn assert_call_lowers_to_assert_instruction() {
    let ir = ir_for(
        r#"
fn main() {
    assert(2 < 3)
}
"#,
    );

    let main = ir.function("main").expect("main function");
    assert!(main.blocks[0]
        .instructions
        .iter()
        .any(|instruction| matches!(instruction, IrInstruction::Assert { .. })));
}

#[test]
fn generic_map_get_in_assert_lowers_to_string_compare() {
    let ir = ir_for(
        r#"
struct Map<K, V> {
    data: Int
}

fn map__get<K, V>(values: &Map<K, V>, key: &Str) -> V
fn string_from(value: &Str) -> String

fn map__get_string_string(values: &Map<String, String>, key: &Str) -> String {
    return string_from("pokemon")
}

fn main() {
    let values: Map<String, String> = Map { data: 0 }
    assert(map__get<String, String>(values, "route") == "pokemon")
}
"#,
    );

    let main = ir.function("main").expect("main function");
    assert!(main.blocks[0].instructions.iter().any(|instruction| {
        matches!(
            instruction,
            IrInstruction::Assert {
                condition: IrBoolExpr::StringCompare {
                    left,
                    ..
                },
                ..
            } if matches!(
                left.as_ref(),
                IrStringExpr::Call { callee, .. } if callee == "map__get_string_string"
            )
        )
    }));
}

#[test]
fn alloc_map_put_string_string_transfer_suppresses_owned_arg_drops() {
    let ir = ir_for(
        r#"
struct Vec<T> {
    data: Int
}

fn alloc__map__put_string_string(keys: &mut Vec<String>, values: &mut Vec<String>, key: String, value: String) -> Int {
    return 1
}

extern "C" {
    fn raw_string_concat(a: Str, b: Str) -> String
}

fn main() {
    let keys: Vec<String> = Vec { data: 0 }
    let values: Vec<String> = Vec { data: 0 }
    let key = raw_string_concat("Content-Type", "")
    let value = raw_string_concat("application/json", "")
    alloc__map__put_string_string(keys, values, key, value)
}
"#,
    );

    let main = ir.function("main").expect("main function");
    let instructions: Vec<_> = main
        .blocks
        .iter()
        .flat_map(|block| block.instructions.iter())
        .collect();

    assert!(!instructions.iter().any(|instruction| {
        matches!(
            instruction,
            IrInstruction::Drop {
                local,
                ty: mo::ir::IrValueTy::OwnedString,
            } if local == "key" || local == "value"
        )
    }));
}

#[test]
fn generic_map_destroy_statement_lowers_to_specialized_call() {
    let ir = ir_for(
        r#"
struct Map<K, V> {
    data: Int
}

fn map__destroy<K, V>(values: &Map<K, V>)

fn map__destroy_string_string(values: &Map<String, String>) {
}

fn main() {
    let values: Map<String, String> = Map { data: 0 }
    map__destroy<String, String>(values)
}
"#,
    );

    let main = ir.function("main").expect("main function");
    assert!(main.blocks[0].instructions.iter().any(|instruction| {
        matches!(
            instruction,
            IrInstruction::Call { callee, .. } if callee == "map__destroy_string_string"
        )
    }));
}

#[test]
fn if_lowers_to_multiple_blocks() {
    let ir = ir_for(
        r#"
fn main(flag: Bool) {
    if flag {
        print("yes")
    }
}
"#,
    );

    let main = ir.function("main").expect("main function");
    assert!(main.blocks.len() >= 3);
    assert!(matches!(
        main.blocks[0].terminator,
        IrTerminator::Branch { .. }
    ));
}

#[test]
fn if_condition_lowers_to_branch_condition() {
    let ir = ir_for(
        r#"
fn main() -> Int {
    if 2 < 3 {
        return 42
    }
    return 1
}
"#,
    );

    let main = ir.function("main").expect("main function");
    assert!(matches!(
        &main.blocks[0].terminator,
        IrTerminator::Branch {
            condition: Some(IrBoolExpr::Compare {
                op: IrCompareOp::Lt,
                ..
            }),
            ..
        }
    ));
}

#[test]
fn assignment_lowers_to_explicit_integer_assign() {
    let ir = ir_for(
        r#"
fn main() -> Int {
    let mut value = 1
    value = value + 2
    return value
}
"#,
    );

    let main = ir.function("main").expect("main function");
    let assignments = main
        .blocks
        .iter()
        .flat_map(|block| block.instructions.iter())
        .filter(|instruction| {
            matches!(instruction, IrInstruction::AssignInt { local, .. } if local == "value")
        })
        .count();
    assert_eq!(assignments, 2);
}

#[test]
fn local_struct_fields_lower_to_struct_assignment_and_field_loads() {
    let ir = ir_for(
        r#"
struct Point {
    x: Int
    y: Int
}

fn main() -> Int {
    let point = Point { x: 20, y: 22 }
    return point.x + point.y
}
"#,
    );

    let main = ir.function("main").expect("main function");
    let instructions: Vec<_> = main
        .blocks
        .iter()
        .flat_map(|block| block.instructions.iter())
        .collect();
    assert!(instructions
        .iter()
        .any(|instruction| matches!(instruction, IrInstruction::AssignStruct { local, value: IrStructExpr::Construct { name, .. } } if local == "point" && name == "Point")));
    assert!(instructions.iter().any(|instruction| {
        matches!(
            instruction,
            IrInstruction::ReturnInt {
                value: IrIntExpr::Binary { left, right, .. }
            } if matches!(left.as_ref(), IrIntExpr::Field { base, field } if base == "point" && field == "x")
                && matches!(right.as_ref(), IrIntExpr::Field { base, field } if base == "point" && field == "y")
        )
    }));
}

#[test]
fn str_struct_fields_lower_as_borrowed_strings() {
    let ir = ir_for(
        r#"
struct View {
    data: Str
    owned: String
}

extern "C" {
    fn raw_string_concat(a: Str, b: Str) -> String
}

fn main() -> Int {
    let view = View { data: "borrowed", owned: raw_string_concat("owned", "") }
    return 42
}
"#,
    );

    let view = ir
        .structs
        .iter()
        .find(|item| item.name == "View")
        .expect("View struct");
    let data = view
        .fields
        .iter()
        .find(|field| field.name == "data")
        .expect("data field");
    let owned = view
        .fields
        .iter()
        .find(|field| field.name == "owned")
        .expect("owned field");

    assert_eq!(data.ty, mo::ir::IrValueTy::BorrowedString);
    assert_eq!(owned.ty, mo::ir::IrValueTy::String);
}

#[test]
fn byte_slice_index_lowers_to_slice_get_call() {
    let ir = ir_for(
        r#"
struct ByteSlice {
    data: Str
    start: Int
    length_value: Int
}

fn slice__get(value: &ByteSlice, index: Int) -> Int {
    return 42
}

fn main() -> Int {
    let whole = ByteSlice { data: "abc", start: 0, length_value: 3 }
    return whole[1]
}
"#,
    );

    let main = ir.function("main").expect("main function");
    assert!(main.blocks.iter().any(|block| {
        block
            .instructions
            .iter()
            .any(|instruction| instruction_calls(instruction, "slice__get"))
            || terminator_calls(&block.terminator, "slice__get")
    }));
}

#[test]
fn vec_field_assignment_lowers_through_generic_vec_type() {
    let ir = ir_for(
        r#"
struct Vec<T> {
    data: Int
    length_value: Int
    capacity_value: Int
}

fn grow(values: &mut Vec<Int>) {
    values.data = 1
    values.capacity_value = 2
}
"#,
    );

    let grow = ir.function("grow").expect("grow function");
    let assignments: Vec<_> = grow
        .blocks
        .iter()
        .flat_map(|block| block.instructions.iter())
        .filter_map(|instruction| {
            if let IrInstruction::AssignField { base, field, .. } = instruction {
                Some((base.as_str(), field.as_str()))
            } else {
                None
            }
        })
        .collect();
    assert_eq!(
        assignments,
        vec![("values", "data"), ("values", "capacity_value")]
    );
}

#[test]
fn option_match_lowers_to_enum_assignment_and_match_return() {
    let ir = ir_for(
        r#"
enum Option<T> {
    Some(T)
    None
}

fn main() -> Int {
    let value: Option<Int> = Some(42)
    return match value {
        Some(x) => x
        None => 0
    }
}
"#,
    );

    let main = ir.function("main").expect("main function");
    let instructions: Vec<_> = main
        .blocks
        .iter()
        .flat_map(|block| block.instructions.iter())
        .collect();
    assert!(instructions.iter().any(|instruction| {
        matches!(
            instruction,
            IrInstruction::AssignEnum {
                local,
                value: IrEnumExpr::Construct { variant, tag, .. }
            } if local == "value" && variant == "Some" && *tag == 0
        )
    }));
    assert!(instructions.iter().any(|instruction| {
        matches!(
            instruction,
            IrInstruction::AssignEnumMatch { arms, .. }
                if arms.len() == 2 && arms[0].variant == "Some" && arms[1].variant == "None"
        )
    }));
    assert!(instructions.iter().any(|instruction| {
        matches!(instruction, IrInstruction::ReturnInt { value: IrIntExpr::Local(local) } if local.starts_with("__return"))
    }));
}

#[test]
fn calls_borrows_and_drops_are_explicit() {
    let ir = ir_for(
        r#"
struct User {
    id: Int
}

fn read(value: &Str) {
}

fn main() {
    let value = "Ada"
    let borrowed = &value
    let user = User { id: 1 }
    read(borrowed)
}
"#,
    );

    let main = ir.function("main").expect("main function");
    let instructions: Vec<_> = main
        .blocks
        .iter()
        .flat_map(|block| block.instructions.iter())
        .collect();

    assert!(instructions
        .iter()
        .any(|instruction| matches!(instruction, IrInstruction::Borrow { .. })));
    assert!(instructions.iter().any(
        |instruction| matches!(instruction, IrInstruction::Call { callee, .. } if callee == "read")
    ));
    assert!(instructions.iter().any(
        |instruction| matches!(instruction, IrInstruction::Drop { local, .. } if local == "user")
    ));
    assert!(!instructions.iter().any(
        |instruction| matches!(instruction, IrInstruction::Drop { local, .. } if local == "value")
    ));
}

#[test]
fn owned_string_local_gets_automatic_drop() {
    let ir = ir_for(
        r#"
fn core__int_to_string(value: Int) -> String {
    return raw_int_to_string(value)
}

fn main() {
    let value = core__int_to_string(42)
}
"#,
    );

    let main = ir.function("main").expect("main function");
    let instructions: Vec<_> = main
        .blocks
        .iter()
        .flat_map(|block| block.instructions.iter())
        .collect();

    assert!(instructions.iter().any(|instruction| {
        matches!(
            instruction,
            IrInstruction::Drop {
                local,
                ty: mo::ir::IrValueTy::OwnedString,
            } if local == "value"
        )
    }));
}

#[test]
fn if_expression_string_value_lowers_to_branch_assignment() {
    let ir = ir_for(
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
    );

    let main = ir.function("main").expect("main function");
    let assign_blocks = main
        .blocks
        .iter()
        .filter(|block| {
            block.instructions.iter().any(|instruction| {
                matches!(
                    instruction,
                    IrInstruction::AssignString { local, .. } if local == "value"
                )
            })
        })
        .count();
    let instructions: Vec<_> = main
        .blocks
        .iter()
        .flat_map(|block| block.instructions.iter())
        .collect();

    assert_eq!(assign_blocks, 2);
    assert!(main
        .blocks
        .iter()
        .any(|block| matches!(block.terminator, IrTerminator::Branch { .. })));
    assert!(instructions.iter().any(|instruction| {
        matches!(
            instruction,
            IrInstruction::Let { local } if local == "value"
        )
    }));
    assert!(instructions.iter().any(|instruction| {
        matches!(
            instruction,
            IrInstruction::Drop {
                local,
                ty: mo::ir::IrValueTy::OwnedString,
            } if local == "value"
        )
    }));
}

#[test]
fn buffer_finish_return_inference_marks_wrapper_result_owned() {
    let ir = ir_for(
        r#"
struct Buffer {
    data: String
}

fn core__alloc_string(capacity: Int) -> String {
    return raw_alloc_string(capacity)
}

fn buffer__new(capacity: Int) -> Buffer {
    return Buffer { data: core__alloc_string(capacity) }
}

fn buffer__finish(buffer: &Buffer) -> String {
    return buffer.data
}

fn response() -> String {
    let out = buffer__new(32)
    return buffer__finish(out)
}

fn main() {
    let value = response()
}
"#,
    );

    let main = ir.function("main").expect("main function");
    let instructions: Vec<_> = main
        .blocks
        .iter()
        .flat_map(|block| block.instructions.iter())
        .collect();

    assert!(instructions.iter().any(|instruction| {
        matches!(
            instruction,
            IrInstruction::Drop {
                local,
                ty: mo::ir::IrValueTy::OwnedString,
            } if local == "value"
        )
    }));
}

#[test]
fn explicit_free_owned_suppresses_automatic_owned_string_drop() {
    let ir = ir_for(
        r#"
fn core__int_to_string(value: Int) -> String {
    return raw_int_to_string(value)
}

fn free_owned(value: &String) {
}

fn main() {
    let value = core__int_to_string(42)
    free_owned(value)
}
"#,
    );

    let main = ir.function("main").expect("main function");
    let instructions: Vec<_> = main
        .blocks
        .iter()
        .flat_map(|block| block.instructions.iter())
        .collect();

    assert!(!instructions.iter().any(|instruction| {
        matches!(
            instruction,
            IrInstruction::Drop {
                local,
                ty: mo::ir::IrValueTy::OwnedString,
            } if local == "value"
        )
    }));
}

#[test]
fn owned_string_return_inference_propagates_through_user_wrappers() {
    let ir = ir_for(
        r#"
fn core__int_to_string(value: Int) -> String {
    return raw_int_to_string(value)
}

fn make_label(value: Int) -> String {
    return core__int_to_string(value)
}

fn main() {
    let label = make_label(42)
}
"#,
    );

    let main = ir.function("main").expect("main function");
    let instructions: Vec<_> = main
        .blocks
        .iter()
        .flat_map(|block| block.instructions.iter())
        .collect();

    assert!(instructions.iter().any(|instruction| {
        matches!(
            instruction,
            IrInstruction::Drop {
                local,
                ty: mo::ir::IrValueTy::OwnedString,
            } if local == "label"
        )
    }));
}

#[test]
fn declared_string_return_from_owned_parameter_stays_owned() {
    let ir = ir_for(
        r#"
extern "C" {
    fn raw_string_concat(a: Str, b: Str) -> String
}

fn pass(value: String) -> String {
    return value
}

fn main() {
    let value: String = raw_string_concat("Ada", "")
    let label = pass(value)
}
"#,
    );

    let main = ir.function("main").expect("main function");
    let instructions: Vec<_> = main
        .blocks
        .iter()
        .flat_map(|block| block.instructions.iter())
        .collect();

    assert!(instructions.iter().any(|instruction| {
        matches!(
            instruction,
            IrInstruction::Drop {
                local,
                ty: mo::ir::IrValueTy::OwnedString,
            } if local == "label"
        )
    }));
}

#[test]
fn declared_string_extern_return_uses_type_fact_for_owned_drop() {
    let ir = ir_for(
        r#"
extern "C" {
    fn make_label() -> String
}

fn main() {
    let value = make_label()
}
"#,
    );

    let main = ir.function("main").expect("main function");
    let instructions: Vec<_> = main
        .blocks
        .iter()
        .flat_map(|block| block.instructions.iter())
        .collect();

    assert!(instructions.iter().any(|instruction| {
        matches!(
            instruction,
            IrInstruction::Drop {
                local,
                ty: mo::ir::IrValueTy::OwnedString,
            } if local == "value"
        )
    }));
}

#[test]
fn declared_str_extern_return_stays_borrowed_in_ir() {
    let ir = ir_for(
        r#"
extern "C" {
    fn borrowed_label() -> Str
}

fn main() {
    let value = borrowed_label()
}
"#,
    );

    let main = ir.function("main").expect("main function");
    let instructions: Vec<_> = main
        .blocks
        .iter()
        .flat_map(|block| block.instructions.iter())
        .collect();

    assert!(!instructions.iter().any(
        |instruction| matches!(instruction, IrInstruction::Drop { local, .. } if local == "value")
    ));
}

#[test]
fn declared_str_wrapper_return_does_not_infer_owned_boundary() {
    let ir = ir_for(
        r#"
extern "C" {
    fn raw_string_concat(a: Str, b: Str) -> String
}

fn borrowed_label() -> Str {
    return raw_string_concat("Ada", "")
}

fn main() {
    let value = borrowed_label()
}
"#,
    );

    let borrowed_label = ir
        .function("borrowed_label")
        .expect("borrowed_label function");
    assert_eq!(
        borrowed_label.return_type,
        mo::ir::IrValueTy::BorrowedString
    );

    let main = ir.function("main").expect("main function");
    let instructions: Vec<_> = main
        .blocks
        .iter()
        .flat_map(|block| block.instructions.iter())
        .collect();

    assert!(!instructions.iter().any(
        |instruction| matches!(instruction, IrInstruction::Drop { local, .. } if local == "value")
    ));
}

#[test]
fn known_owned_string_name_declared_str_return_stays_borrowed() {
    let ir = ir_for(
        r#"
extern "C" {
    fn concat() -> Str
}

fn main() {
    let value = concat()
}
"#,
    );

    let concat = ir
        .extern_functions
        .iter()
        .find(|function| function.name == "concat")
        .expect("concat extern");
    assert_eq!(concat.return_type, mo::ir::IrValueTy::BorrowedString);

    let main = ir.function("main").expect("main function");
    let instructions: Vec<_> = main
        .blocks
        .iter()
        .flat_map(|block| block.instructions.iter())
        .collect();

    assert!(!instructions.iter().any(
        |instruction| matches!(instruction, IrInstruction::Drop { local, .. } if local == "value")
    ));
}

#[test]
fn user_function_named_like_owned_string_intrinsic_declared_str_stays_borrowed() {
    let ir = ir_for(
        r#"
fn from() -> Str {
    return "borrowed"
}

fn main() {
    let value = from()
}
"#,
    );

    let from = ir.function("from").expect("from function");
    assert_eq!(from.return_type, mo::ir::IrValueTy::BorrowedString);

    let main = ir.function("main").expect("main function");
    let instructions: Vec<_> = main
        .blocks
        .iter()
        .flat_map(|block| block.instructions.iter())
        .collect();

    assert!(!instructions.iter().any(
        |instruction| matches!(instruction, IrInstruction::Drop { local, .. } if local == "value")
    ));
}

#[test]
fn result_or_else_like_generic_match_frees_input_enum_storage_on_return() {
    let ir = ir_for(
        r#"
enum Result<T, E> {
    Ok(T)
    Err(E)
}

fn or_else<T, E, F>(value: Result<T, E>, fallback: fn(E) -> Result<T, F>) -> Result<T, F> {
    return match value {
        Ok(item) => Ok(item)
        Err(error) => fallback(error)
    }
}
"#,
    );

    let function = ir.function("or_else").expect("or_else function");
    assert!(function.blocks.iter().any(|block| {
        block.instructions.iter().any(|instruction| {
            matches!(
                instruction,
                IrInstruction::ReturnEnumMatch {
                    free_value_storage: true,
                    ..
                }
            )
        })
    }));
}

#[test]
fn enum_return_constructed_from_owned_read_drops_before_return() {
    let ir = ir_for(
        r#"
enum Result<T, E> {
    Ok(T)
    Err(E)
}

fn inspect(value: &Str) -> Int {
    return 42
}

fn len(value: String) -> Result<Int, Int> {
    return Ok(inspect(value))
}
"#,
    );

    let function = ir.function("len").expect("len function");
    let instructions: Vec<_> = function
        .blocks
        .iter()
        .flat_map(|block| block.instructions.iter())
        .collect();
    let drop_index = instructions
        .iter()
        .position(|instruction| {
            matches!(
                instruction,
                IrInstruction::Drop {
                    local,
                    ty: mo::ir::IrValueTy::OwnedString,
                } if local == "value"
            )
        })
        .expect("owned parameter drop");
    let return_index = instructions
        .iter()
        .position(|instruction| matches!(instruction, IrInstruction::ReturnEnum { .. }))
        .expect("enum return");

    assert!(drop_index < return_index);
}

#[test]
fn struct_return_constructed_from_owned_read_drops_before_return() {
    let ir = ir_for(
        r#"
extern "C" {
    fn raw_string_concat(a: Str, b: Str) -> String
}

struct Report {
    len: Int
    copy: String
}

fn inspect(value: &Str) -> Int {
    return 2
}

fn make(value: String) -> Report {
    return Report {
        len: inspect(value)
        copy: raw_string_concat("ok", "")
    }
}
"#,
    );

    let function = ir.function("make").expect("make function");
    let instructions: Vec<_> = function
        .blocks
        .iter()
        .flat_map(|block| block.instructions.iter())
        .collect();

    let assign_index = instructions
        .iter()
        .position(|instruction| {
            matches!(
                instruction,
                IrInstruction::AssignStruct {
                    local,
                    value: IrStructExpr::Construct { .. },
                } if local.starts_with("__return")
            )
        })
        .expect("return temp assignment");
    let return_temp = match instructions[assign_index] {
        IrInstruction::AssignStruct { local, .. } => local,
        _ => unreachable!(),
    };
    let drop_index = instructions
        .iter()
        .position(|instruction| {
            matches!(
                instruction,
                IrInstruction::Drop {
                    local,
                    ty: mo::ir::IrValueTy::OwnedString,
                } if local == "value"
            )
        })
        .expect("owned parameter drop");
    let return_index = instructions
        .iter()
        .position(|instruction| {
            matches!(
                instruction,
                IrInstruction::ReturnStruct {
                    value: IrStructExpr::Local(local),
                } if local == return_temp
            )
        })
        .expect("return temp");

    assert!(assign_index < drop_index);
    assert!(drop_index < return_index);
}

#[test]
fn scalar_return_reading_owned_struct_drops_struct_before_return() {
    let ir = ir_for(
        r#"
extern "C" {
    fn raw_string_concat(a: Str, b: Str) -> String
}

struct Report {
    len: Int
    copy: String
}

fn main() -> Int {
    let report = Report {
        len: 42
        copy: raw_string_concat("ok", "")
    }
    return report.len
}
"#,
    );

    let function = ir.function("main").expect("main function");
    let instructions: Vec<_> = function
        .blocks
        .iter()
        .flat_map(|block| block.instructions.iter())
        .collect();

    let assign_index = instructions
        .iter()
        .position(|instruction| {
            matches!(
                instruction,
                IrInstruction::AssignInt {
                    local,
                    value: IrIntExpr::Field { base, field },
                } if local.starts_with("__return") && base == "report" && field == "len"
            )
        })
        .expect("return temp assignment");
    let return_temp = match instructions[assign_index] {
        IrInstruction::AssignInt { local, .. } => local,
        _ => unreachable!(),
    };
    let drop_index = instructions
        .iter()
        .position(|instruction| {
            matches!(
                instruction,
                IrInstruction::Drop {
                    local,
                    ty: mo::ir::IrValueTy::Struct(_),
                } if local == "report"
            )
        })
        .expect("owned struct drop");
    let return_index = instructions
        .iter()
        .position(|instruction| {
            matches!(
                instruction,
                IrInstruction::ReturnInt {
                    value: IrIntExpr::Local(local),
                } if local == return_temp
            )
        })
        .expect("return temp");

    assert!(assign_index < drop_index);
    assert!(drop_index < return_index);
}

#[test]
fn function_return_reading_owned_struct_drops_struct_before_return() {
    let ir = ir_for(
        r#"
extern "C" {
    fn raw_string_concat(a: Str, b: Str) -> String
}

struct Handler {
    label: String
    callback: fn(Int) -> Int
}

fn inc(value: Int) -> Int {
    return value + 1
}

fn choose() -> fn(Int) -> Int {
    let handler = Handler {
        label: raw_string_concat("owned", "")
        callback: inc
    }
    return handler.callback
}
"#,
    );

    let function = ir.function("choose").expect("choose function");
    let instructions: Vec<_> = function
        .blocks
        .iter()
        .flat_map(|block| block.instructions.iter())
        .collect();

    let assign_index = instructions
        .iter()
        .position(|instruction| {
            matches!(
                instruction,
                IrInstruction::AssignFunction {
                    local,
                    value: IrFunctionExpr::Field { base, field },
                } if local.starts_with("__return") && base == "handler" && field == "callback"
            )
        })
        .expect("return temp assignment");
    let return_temp = match instructions[assign_index] {
        IrInstruction::AssignFunction { local, .. } => local,
        _ => unreachable!(),
    };
    let drop_index = instructions
        .iter()
        .position(|instruction| {
            matches!(
                instruction,
                IrInstruction::Drop {
                    local,
                    ty: mo::ir::IrValueTy::Struct(_),
                } if local == "handler"
            )
        })
        .expect("owned struct drop");
    let return_index = instructions
        .iter()
        .position(|instruction| {
            matches!(
                instruction,
                IrInstruction::ReturnFunction {
                    value: IrFunctionExpr::Local(local),
                } if local == return_temp
            )
        })
        .expect("return temp");

    assert!(assign_index < drop_index);
    assert!(drop_index < return_index);
}

#[test]
fn string_block_expression_lowers_to_assignment() {
    let ir = ir_for(
        r#"
extern "C" {
    fn raw_string_concat(a: Str, b: Str) -> String
}

fn main() {
    let value = {
        raw_string_concat("Ada", "")
    }
}
"#,
    );

    let main = ir.function("main").expect("main function");
    let instructions: Vec<_> = main
        .blocks
        .iter()
        .flat_map(|block| block.instructions.iter())
        .collect();

    assert!(instructions.iter().any(|instruction| {
        matches!(
            instruction,
            IrInstruction::AssignString { local, .. } if local == "value"
        )
    }));
    assert!(instructions.iter().any(|instruction| {
        matches!(
            instruction,
            IrInstruction::Drop {
                local,
                ty: mo::ir::IrValueTy::OwnedString,
            } if local == "value"
        )
    }));
}

#[test]
fn return_string_block_expression_lowers_to_return_string() {
    let ir = ir_for(
        r#"
extern "C" {
    fn raw_string_concat(a: Str, b: Str) -> String
}

fn main() -> String {
    return {
        raw_string_concat("Ada", "")
    }
}
"#,
    );

    let main = ir.function("main").expect("main function");
    let instructions: Vec<_> = main
        .blocks
        .iter()
        .flat_map(|block| block.instructions.iter())
        .collect();

    assert!(instructions
        .iter()
        .any(|instruction| matches!(instruction, IrInstruction::ReturnString { .. })));
}

#[test]
fn by_value_string_parameter_is_owned_and_dropped() {
    let ir = ir_for(
        r#"
extern "C" {
    fn raw_string_concat(a: Str, b: Str) -> String
}

fn consume(value: String) {
}

fn main() {
    let value = raw_string_concat("Ada", "")
    consume(value)
}
"#,
    );

    let consume = ir.function("consume").expect("consume function");
    assert!(matches!(
        consume.param_types.as_slice(),
        [mo::ir::IrValueTy::OwnedString]
    ));
    assert!(consume.blocks.iter().any(|block| {
        block.instructions.iter().any(|instruction| {
            matches!(
                instruction,
                IrInstruction::Drop {
                    local,
                    ty: mo::ir::IrValueTy::OwnedString,
                } if local == "value"
            )
        })
    }));
}

#[test]
fn extern_string_reference_parameter_is_borrowed() {
    let ir = ir_for(
        r#"
extern "C" {
    fn inspect(value: &String) -> Int
}

fn len(value: &String) -> Int {
    return 1
}

fn main(value: String) -> Int {
    let size = inspect(value)
    return size + len(value)
}
"#,
    );

    let inspect = ir
        .extern_functions
        .iter()
        .find(|function| function.name == "inspect")
        .expect("inspect extern");
    assert!(matches!(
        inspect.param_types.as_slice(),
        [mo::ir::IrValueTy::BorrowedString]
    ));
}

#[test]
fn string_transfer_parameter_is_not_auto_dropped_after_storage() {
    let ir = ir_for(
        r#"
fn core__store64(ptr: Int, offset: Int, value: Int) {
}

fn core__string_ptr(value: &String) -> Int {
    return 0
}

fn alloc_vec__store_string(data: Int, index: Int, value: String) {
    core__store64(data, index * 8, core__string_ptr(value))
}
"#,
    );

    let store = ir
        .function("alloc_vec__store_string")
        .expect("store function");
    assert!(!store.blocks.iter().any(|block| {
        block.instructions.iter().any(|instruction| {
            matches!(
                instruction,
                IrInstruction::Drop {
                    local,
                    ty: mo::ir::IrValueTy::OwnedString,
                } if local == "value"
            )
        })
    }));
}

#[test]
fn borrowed_string_return_is_not_treated_as_owned() {
    let ir = ir_for(
        r#"
fn borrow_string(value: &Str) -> Str {
    return value
}

fn main() {
    let literal = "Ada"
    let borrowed = borrow_string(&literal)
}
"#,
    );

    let main = ir.function("main").expect("main function");
    let instructions: Vec<_> = main
        .blocks
        .iter()
        .flat_map(|block| block.instructions.iter())
        .collect();

    assert!(!instructions.iter().any(|instruction| {
        matches!(
            instruction,
            IrInstruction::Drop {
                local,
                ty: mo::ir::IrValueTy::OwnedString,
            } if local == "borrowed"
        )
    }));
}

#[test]
fn direct_return_call_using_owned_resource_drops_before_return() {
    let ir = ir_for(
        r#"
struct buffer__Buffer {
    data: String
}

fn buffer__destroy(buffer: &buffer__Buffer) {
}

fn buffer__length(buffer: &buffer__Buffer) -> Int {
    return 3
}

extern "C" {
    fn raw_string_concat(a: Str, b: Str) -> String
}

fn main() -> Int {
    let buffer = buffer__Buffer { data: raw_string_concat("hello", "") }
    return buffer__length(buffer)
}
"#,
    );

    let main = ir.function("main").expect("main function");
    let instructions: Vec<_> = main
        .blocks
        .iter()
        .flat_map(|block| block.instructions.iter())
        .collect();
    let drop_index = instructions
        .iter()
        .position(|instruction| {
            matches!(instruction, IrInstruction::Drop { local, ty: mo::ir::IrValueTy::Struct(name) } if local == "buffer" && name == "buffer__Buffer")
        })
        .expect("buffer drop");
    let return_index = instructions
        .iter()
        .position(|instruction| matches!(instruction, IrInstruction::ReturnInt { .. }))
        .expect("return int");

    assert!(drop_index < return_index);
    assert!(instructions.iter().any(|instruction| {
        matches!(instruction, IrInstruction::AssignInt { local, .. } if local.starts_with("__return"))
    }));
}

#[test]
fn string_builder_resource_drops_before_return() {
    let ir = ir_for(
        r#"
struct buffer__StringBuilder {
    data: String
}

fn buffer__string_builder_destroy(builder: &buffer__StringBuilder) {
}

fn buffer__string_builder_length(builder: &buffer__StringBuilder) -> Int {
    return 3
}

extern "C" {
    fn raw_string_concat(a: Str, b: Str) -> String
}

fn main() -> Int {
    let builder = buffer__StringBuilder { data: raw_string_concat("hello", "") }
    return buffer__string_builder_length(builder)
}
"#,
    );

    let main = ir.function("main").expect("main function");
    let instructions: Vec<_> = main
        .blocks
        .iter()
        .flat_map(|block| block.instructions.iter())
        .collect();
    let drop_index = instructions
        .iter()
        .position(|instruction| {
            matches!(instruction, IrInstruction::Drop { local, ty: mo::ir::IrValueTy::Struct(name) } if local == "builder" && name == "buffer__StringBuilder")
        })
        .expect("builder drop");
    let return_index = instructions
        .iter()
        .position(|instruction| matches!(instruction, IrInstruction::ReturnInt { .. }))
        .expect("return int");

    assert!(drop_index < return_index);
}

#[test]
fn byte_buffer_resource_drops_before_return() {
    let ir = ir_for(
        r#"
struct buffer__ByteBuffer {
    data: String
}

fn buffer__byte_buffer_destroy(bytes: &buffer__ByteBuffer) {
}

fn buffer__byte_buffer_length(bytes: &buffer__ByteBuffer) -> Int {
    return 2
}

extern "C" {
    fn raw_string_concat(a: Str, b: Str) -> String
}

fn main() -> Int {
    let bytes = buffer__ByteBuffer { data: raw_string_concat("hi", "") }
    return buffer__byte_buffer_length(bytes)
}
"#,
    );

    let main = ir.function("main").expect("main function");
    let instructions: Vec<_> = main
        .blocks
        .iter()
        .flat_map(|block| block.instructions.iter())
        .collect();
    let drop_index = instructions
        .iter()
        .position(|instruction| {
            matches!(instruction, IrInstruction::Drop { local, ty: mo::ir::IrValueTy::Struct(name) } if local == "bytes" && name == "buffer__ByteBuffer")
        })
        .expect("byte buffer drop");
    let return_index = instructions
        .iter()
        .position(|instruction| matches!(instruction, IrInstruction::ReturnInt { .. }))
        .expect("return int");

    assert!(drop_index < return_index);
}

#[test]
fn branch_local_owned_strings_drop_before_early_return() {
    let ir = ir_for(
        r#"
extern "C" {
    fn raw_string_concat(a: Str, b: Str) -> String
}

fn main(flag: Bool) -> Int {
    let outer = raw_string_concat("Root", "")
    if flag {
        let first = raw_string_concat("Ada", "")
        let second = raw_string_concat("Grace", "")
        return 1
    }
    return 0
}
"#,
    );

    let main = ir.function("main").expect("main function");
    let instructions = main
        .blocks
        .iter()
        .flat_map(|block| block.instructions.iter())
        .collect::<Vec<_>>();
    let second_drop = instructions
        .iter()
        .position(|instruction| {
            matches!(
                instruction,
                IrInstruction::Drop {
                    local,
                    ty: mo::ir::IrValueTy::OwnedString,
                } if local == "second"
            )
        })
        .expect("second drop before return");
    let first_drop = instructions
        .iter()
        .position(|instruction| {
            matches!(
                instruction,
                IrInstruction::Drop {
                    local,
                    ty: mo::ir::IrValueTy::OwnedString,
                } if local == "first"
            )
        })
        .expect("first drop before return");
    let outer_drop = instructions
        .iter()
        .position(|instruction| {
            matches!(
                instruction,
                IrInstruction::Drop {
                    local,
                    ty: mo::ir::IrValueTy::OwnedString,
                } if local == "outer"
            )
        })
        .expect("outer drop before return");
    let early_return = instructions
        .iter()
        .position(|instruction| {
            matches!(
                instruction,
                IrInstruction::ReturnInt {
                    value: mo::ir::IrIntExpr::Const(1)
                }
            )
        })
        .expect("early return");

    assert!(second_drop < first_drop);
    assert!(first_drop < outer_drop);
    assert!(outer_drop < early_return);
}

#[test]
fn if_branch_fallthrough_drops_branch_local_owned_strings() {
    let ir = ir_for(
        r#"
extern "C" {
    fn raw_string_concat(a: Str, b: Str) -> String
}

fn main(flag: Bool) -> Int {
    if flag {
        let inner = raw_string_concat("branch", "")
    }
    return 42
}
"#,
    );

    let main = ir.function("main").expect("main function");
    let branch_block = main
        .blocks
        .iter()
        .find(|block| {
            block.instructions.iter().any(|instruction| {
                matches!(
                    instruction,
                    IrInstruction::Drop {
                        local,
                        ty: mo::ir::IrValueTy::OwnedString,
                    } if local == "inner"
                )
            })
        })
        .expect("branch block drops inner");
    let drop_index = branch_block
        .instructions
        .iter()
        .position(|instruction| {
            matches!(
                instruction,
                IrInstruction::Drop {
                    local,
                    ty: mo::ir::IrValueTy::OwnedString,
                } if local == "inner"
            )
        })
        .expect("inner drop");
    assert_eq!(drop_index, branch_block.instructions.len() - 1);
}

#[test]
fn nested_block_local_owned_string_drops_before_early_return() {
    let ir = ir_for(
        r#"
extern "C" {
    fn raw_string_concat(a: Str, b: Str) -> String
}

fn main() -> Int {
    let outer = raw_string_concat("Root", "")
    let value = {
        let inner = raw_string_concat("Ada", "")
        return 1
    }
    return value
}
"#,
    );

    let main = ir.function("main").expect("main function");
    let instructions = main
        .blocks
        .iter()
        .flat_map(|block| block.instructions.iter())
        .collect::<Vec<_>>();
    let inner_drop = instructions
        .iter()
        .position(|instruction| {
            matches!(
                instruction,
                IrInstruction::Drop {
                    local,
                    ty: mo::ir::IrValueTy::OwnedString,
                } if local == "inner"
            )
        })
        .expect("inner drop before return");
    let outer_drop = instructions
        .iter()
        .position(|instruction| {
            matches!(
                instruction,
                IrInstruction::Drop {
                    local,
                    ty: mo::ir::IrValueTy::OwnedString,
                } if local == "outer"
            )
        })
        .expect("outer drop before return");
    let early_return = instructions
        .iter()
        .position(|instruction| {
            matches!(
                instruction,
                IrInstruction::ReturnInt {
                    value: mo::ir::IrIntExpr::Const(1)
                }
            )
        })
        .expect("early return");

    assert!(inner_drop < outer_drop);
    assert!(outer_drop < early_return);
}

#[test]
fn return_if_expression_drops_branch_owned_locals_before_return() {
    let ir = ir_for(
        r#"
extern "C" {
    fn raw_string_concat(a: Str, b: Str) -> String
    fn raw_strlen(value: Str) -> Int
}

fn main(flag: Bool) -> Int {
    return if flag {
        let inner = raw_string_concat("Ada", "")
        raw_strlen(inner)
    } else {
        let fallback = raw_string_concat("Grace", "")
        raw_strlen(fallback)
    }
}
"#,
    );

    let main = ir.function("main").expect("main function");
    let branch_blocks = main
        .blocks
        .iter()
        .filter(|block| {
            block
                .instructions
                .iter()
                .any(|instruction| matches!(instruction, IrInstruction::ReturnInt { .. }))
        })
        .collect::<Vec<_>>();

    assert_eq!(branch_blocks.len(), 2);
    for (local, block) in [("inner", branch_blocks[0]), ("fallback", branch_blocks[1])] {
        let drop_index = block
            .instructions
            .iter()
            .position(|instruction| {
                matches!(
                    instruction,
                    IrInstruction::Drop {
                        local: dropped,
                        ty: mo::ir::IrValueTy::OwnedString,
                    } if dropped == local
                )
            })
            .expect("branch local drop");
        let return_index = block
            .instructions
            .iter()
            .position(|instruction| matches!(instruction, IrInstruction::ReturnInt { .. }))
            .expect("branch return");

        assert!(drop_index < return_index);
    }
}

#[test]
fn loop_exit_drops_body_owned_locals_before_jump() {
    let ir = ir_for(
        r#"
extern "C" {
    fn raw_string_concat(a: Str, b: Str) -> String
}

fn main() -> Int {
    let mut count = 0
    while count < 1 {
        let inner = raw_string_concat("Ada", "")
        count = count + 1
        break
    }
    return count
}
"#,
    );

    let main = ir.function("main").expect("main function");
    let instructions = main
        .blocks
        .iter()
        .flat_map(|block| block.instructions.iter())
        .collect::<Vec<_>>();
    assert!(instructions.iter().any(|instruction| {
        matches!(
            instruction,
            IrInstruction::Drop {
                local,
                ty: mo::ir::IrValueTy::OwnedString,
            } if local == "inner"
        )
    }));
}

#[test]
fn match_arm_return_keeps_drops_and_return_terminator() {
    let ir = ir_for(
        r#"
extern "C" {
    fn raw_string_concat(a: Str, b: Str) -> String
}

enum Choice {
    First
    Second
}

fn main() -> Int {
    let outer = raw_string_concat("Root", "")
    let choice: Choice = First
    match choice {
        First => {
            let inner = raw_string_concat("Ada", "")
            return 1
        }
        Second => 0
    }
    return 0
}
"#,
    );

    let main = ir.function("main").expect("main function");
    let return_block = main
        .blocks
        .iter()
        .find(|block| {
            block.instructions.iter().any(|instruction| {
                matches!(
                    instruction,
                    IrInstruction::ReturnInt {
                        value: mo::ir::IrIntExpr::Const(1)
                    }
                )
            })
        })
        .expect("match arm return block");
    let instructions = return_block.instructions.iter().collect::<Vec<_>>();
    let inner_drop = instructions
        .iter()
        .position(|instruction| {
            matches!(
                instruction,
                IrInstruction::Drop {
                    local,
                    ty: mo::ir::IrValueTy::OwnedString,
                } if local == "inner"
            )
        })
        .expect("inner drop before arm return");
    let outer_drop = instructions
        .iter()
        .position(|instruction| {
            matches!(
                instruction,
                IrInstruction::Drop {
                    local,
                    ty: mo::ir::IrValueTy::OwnedString,
                } if local == "outer"
            )
        })
        .expect("outer drop before arm return");
    let return_index = instructions
        .iter()
        .position(|instruction| matches!(instruction, IrInstruction::ReturnInt { .. }))
        .expect("arm return");

    assert!(inner_drop < outer_drop);
    assert!(outer_drop < return_index);
    assert!(matches!(return_block.terminator, IrTerminator::Return));
}

#[test]
fn buffer_finish_suppresses_ir_buffer_drop() {
    let ir = ir_for(
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
    );

    let main = ir.function("main").expect("main function");
    let instructions: Vec<_> = main
        .blocks
        .iter()
        .flat_map(|block| block.instructions.iter())
        .collect();

    assert!(!instructions.iter().any(|instruction| {
        matches!(instruction, IrInstruction::Drop { local, ty: mo::ir::IrValueTy::Struct(_) } if local == "buffer")
    }));
}

#[test]
fn string_builder_finish_suppresses_ir_drop() {
    let ir = ir_for(
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
    );

    let main = ir.function("main").expect("main function");
    let instructions: Vec<_> = main
        .blocks
        .iter()
        .flat_map(|block| block.instructions.iter())
        .collect();

    assert!(!instructions.iter().any(|instruction| {
        matches!(instruction, IrInstruction::Drop { local, ty: mo::ir::IrValueTy::Struct(_) } if local == "builder")
    }));
}

#[test]
fn byte_buffer_finish_suppresses_ir_drop() {
    let ir = ir_for(
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
    let bytes = buffer__ByteBuffer { data: raw_string_concat("hi", "") }
    let out = buffer__byte_buffer_finish(bytes)
}
"#,
    );

    let main = ir.function("main").expect("main function");
    let instructions: Vec<_> = main
        .blocks
        .iter()
        .flat_map(|block| block.instructions.iter())
        .collect();

    assert!(!instructions.iter().any(|instruction| {
        matches!(instruction, IrInstruction::Drop { local, ty: mo::ir::IrValueTy::Struct(_) } if local == "bytes")
    }));
}

#[test]
fn explicit_task_queue_destroy_suppresses_ir_drop() {
    let ir = ir_for(
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
    );

    let main = ir.function("main").expect("main function");
    let instructions: Vec<_> = main
        .blocks
        .iter()
        .flat_map(|block| block.instructions.iter())
        .collect();

    assert!(!instructions.iter().any(|instruction| {
        matches!(instruction, IrInstruction::Drop { local, .. } if local == "queue")
    }));
}

#[test]
fn drop_interface_impl_is_recorded_for_backend_drop_glue() {
    let ir = ir_for(
        r#"
interface Drop {
    fn drop(&self)
}

struct Manual: Drop {
    data: String

    fn drop(&self) {
    }
}

extern "C" {
    fn raw_string_concat(a: Str, b: Str) -> String
}

fn main() {
    let value = Manual { data: raw_string_concat("hello", "") }
}
"#,
    );

    assert_eq!(ir.drop_impls.get("Manual"), Some(&"drop".to_string()));
}

#[test]
fn typed_tcp_owners_emit_resource_drops() {
    let ir = ir_for(
        r#"
struct net__TcpListener {
    fd: Int
}

struct net__TcpStream {
    fd: Int
}

fn main() {
    let listener = net__TcpListener { fd: 1 }
    let stream = net__TcpStream { fd: 2 }
}
"#,
    );

    let main = ir.function("main").expect("main function");
    let instructions: Vec<_> = main
        .blocks
        .iter()
        .flat_map(|block| block.instructions.iter())
        .collect();

    assert!(instructions.iter().any(
        |instruction| matches!(instruction, IrInstruction::Drop { local, ty: mo::ir::IrValueTy::Struct(name) } if local == "stream" && name == "net__TcpStream")
    ));
    assert!(instructions.iter().any(
        |instruction| matches!(instruction, IrInstruction::Drop { local, ty: mo::ir::IrValueTy::Struct(name) } if local == "listener" && name == "net__TcpListener")
    ));
}

#[test]
fn explicit_typed_tcp_close_suppresses_ir_drop() {
    let ir = ir_for(
        r#"
struct net__TcpListener {
    fd: Int
}

struct net__TcpStream {
    fd: Int
}

fn net__tcp_listener_close(listener: &net__TcpListener) -> Int {
    return 0
}

fn net__tcp_stream_close(stream: &net__TcpStream) -> Int {
    return 0
}

fn main() {
    let listener = net__TcpListener { fd: 1 }
    let stream = net__TcpStream { fd: 2 }
    net__tcp_stream_close(stream)
    net__tcp_listener_close(listener)
}
"#,
    );

    let main = ir.function("main").expect("main function");
    let instructions: Vec<_> = main
        .blocks
        .iter()
        .flat_map(|block| block.instructions.iter())
        .collect();

    assert!(!instructions.iter().any(
        |instruction| matches!(instruction, IrInstruction::Drop { local, .. } if local == "stream")
    ));
    assert!(!instructions.iter().any(
        |instruction| matches!(instruction, IrInstruction::Drop { local, .. } if local == "listener")
    ));
}

#[test]
fn channel_shared_handle_emits_wrapper_ir_drop() {
    let ir = ir_for(
        r#"
struct channel__Channel {
    raw: Int
}

fn main() {
    let ch = channel__Channel { raw: 0 }
}
"#,
    );

    let main = ir.function("main").expect("main function");
    let instructions: Vec<_> = main
        .blocks
        .iter()
        .flat_map(|block| block.instructions.iter())
        .collect();

    assert!(instructions.iter().any(|instruction| {
        matches!(instruction, IrInstruction::Drop { local, ty: mo::ir::IrValueTy::Struct(name) } if local == "ch" && name == "channel__Channel")
    }));
}

#[test]
fn enum_match_can_assign_to_integer_local() {
    let ir = ir_for(
        r#"
enum Result<T, E> {
    Ok(T)
    Err(E)
}

fn main() -> Int {
    let result: Result<Int, Int> = Ok(41)
    let code: Int = match result {
        Ok(value) => value + 1
        Err(error) => error
    }
    return code
}
"#,
    );

    let main = ir.function("main").expect("main function");
    let instructions: Vec<_> = main
        .blocks
        .iter()
        .flat_map(|block| block.instructions.iter())
        .collect();
    assert!(instructions.iter().any(|instruction| {
        matches!(
            instruction,
            IrInstruction::AssignEnumMatch { local, arms, .. }
                if local == "code" && arms.len() == 2 && arms[0].variant == "Ok" && arms[1].variant == "Err"
        )
    }));
}

#[test]
fn while_loop_lowers_to_condition_branch_and_backedge() {
    let ir = ir_for(
        r#"
fn main(flag: Bool) {
    while flag {
        print("tick")
    }
}
"#,
    );

    let main = ir.function("main").expect("main function");
    assert!(main
        .blocks
        .iter()
        .any(|block| matches!(block.terminator, IrTerminator::Branch { .. })));
    assert!(main.blocks.iter().any(|block| {
        matches!(block.terminator, IrTerminator::Jump { target } if target <= block.id)
    }));
}

#[test]
fn loop_break_and_continue_lower_to_jumps() {
    let ir = ir_for(
        r#"
fn main(flag: Bool) {
    loop {
        if flag {
            break
        }
        continue
    }
}
"#,
    );

    let main = ir.function("main").expect("main function");
    let jumps: Vec<_> = main
        .blocks
        .iter()
        .filter_map(|block| match block.terminator {
            IrTerminator::Jump { target } => Some((block.id, target)),
            _ => None,
        })
        .collect();
    assert!(jumps.iter().any(|(id, target)| target > id));
    assert!(jumps.iter().any(|(id, target)| target <= id));
}

#[test]
fn match_lowers_to_switch_blocks() {
    let ir = ir_for(
        r#"
enum Option {
    Some(String)
    None
}

fn main(value: Option) {
    match value {
        Some(name) => print(name)
        None => print("none")
    }
}
"#,
    );

    let main = ir.function("main").expect("main function");
    assert!(main.blocks.iter().any(|block| matches!(
        &block.terminator,
        IrTerminator::Switch { arms, .. } if arms.len() == 2 && arms[0].tag == 0 && arms[1].tag == 1
    )));
    assert!(main.blocks.iter().any(|block| {
        block.instructions.iter().any(|instruction| {
            matches!(
                instruction,
                IrInstruction::BindEnumPayload {
                    local,
                    payload_index: 0,
                    ..
                } if local == "name"
            )
        })
    }));
}

#[test]
fn try_expression_lowers_to_result_branch() {
    let ir = ir_for(
        r#"
enum Result<T, E> {
    Ok(T)
    Err(E)
}

extern "C" {
    fn raw_string_concat(a: Str, b: Str) -> String
}

fn source() -> Result<String, Error> {
}

fn main() -> Result<String, Error> {
    let outer = raw_string_concat("Root", "")
    let value = source()?
    return Ok(value)
}
"#,
    );

    let main = ir.function("main").expect("main function");
    let instructions: Vec<_> = main
        .blocks
        .iter()
        .flat_map(|block| block.instructions.iter())
        .collect();

    assert!(instructions
        .iter()
        .any(|instruction| matches!(instruction, IrInstruction::Try { .. })));
    assert!(main
        .blocks
        .iter()
        .any(|block| matches!(block.terminator, IrTerminator::Branch { .. })));
    let error_block = main
        .blocks
        .iter()
        .find(|block| {
            matches!(block.terminator, IrTerminator::Return)
                && block.instructions.iter().any(|instruction| {
                    matches!(
                        instruction,
                        IrInstruction::Drop {
                            local,
                            ty: mo::ir::IrValueTy::OwnedString,
                        } if local == "outer"
                    )
                })
        })
        .expect("try error path drops outer before return");
    let drop_index = error_block
        .instructions
        .iter()
        .position(|instruction| {
            matches!(
                instruction,
                IrInstruction::Drop {
                    local,
                    ty: mo::ir::IrValueTy::OwnedString,
                } if local == "outer"
            )
        })
        .expect("outer drop");
    let return_index = error_block
        .instructions
        .iter()
        .position(|instruction| matches!(instruction, IrInstruction::ReturnEnum { .. }))
        .expect("propagated error return");
    assert!(drop_index < return_index);
}

#[test]
fn async_function_records_future_state() {
    let ir = ir_for(
        r#"
extern "C" {
    fn raw_string_concat(a: Str, b: Str) -> String
}

async fn load() -> String {
    return raw_string_concat("Ada", "")
}
"#,
    );

    let load = ir.function("load").expect("load function");
    assert!(load.is_async);
    assert!(load.future_state.is_some());
}

#[test]
fn await_expression_creates_suspend_point() {
    let ir = ir_for(
        r#"
extern "C" {
    fn raw_string_concat(a: Str, b: Str) -> String
}

async fn fetch() -> String {
    return raw_string_concat("Ada", "")
}

async fn main() -> String {
    let value = fetch().await
    return value
}
"#,
    );

    let main = ir.function("main").expect("main function");
    let instructions: Vec<_> = main
        .blocks
        .iter()
        .flat_map(|block| block.instructions.iter())
        .collect();
    assert!(instructions
        .iter()
        .any(|instruction| matches!(instruction, IrInstruction::Suspend { .. })));
}

#[test]
fn async_closure_records_captured_locals() {
    let ir = ir_for(
        r#"
fn main() {
    let message = "Ada"
    let handler = async fn() {
        print(message)
    }
}
"#,
    );

    let main = ir.function("main").expect("main function");
    let instructions: Vec<_> = main
        .blocks
        .iter()
        .flat_map(|block| block.instructions.iter())
        .collect();
    assert!(instructions.iter().any(
        |instruction| matches!(instruction, IrInstruction::FutureState { captures } if captures == &vec!["message".to_string()])
    ));
}

#[test]
fn borrow_across_await_is_represented() {
    let ir = ir_for(
        r#"
async fn wait() {
}

async fn main() {
    let value = "Ada"
    let borrowed = &value
    wait().await
    print(borrowed)
}
"#,
    );

    let main = ir.function("main").expect("main function");
    let instructions: Vec<_> = main
        .blocks
        .iter()
        .flat_map(|block| block.instructions.iter())
        .collect();
    assert!(instructions.iter().any(
        |instruction| matches!(instruction, IrInstruction::BorrowAcrossSuspend { local } if local == "borrowed")
    ));
}
