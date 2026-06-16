use crate::ast::*;
use crate::dropck::DropReport;
use crate::hir::{HirFunction, HirModuleId, HirProgram};
use crate::resource::{is_shared_handle_name, is_unique_resource_name};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrProgram {
    pub modules: Vec<IrModule>,
    pub extern_functions: Vec<IrExternFunction>,
    pub structs: Vec<IrStruct>,
    pub enums: Vec<IrEnum>,
    pub functions: Vec<IrFunction>,
    pub drop_impls: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrModule {
    pub id: HirModuleId,
    pub path: Vec<String>,
}

impl IrProgram {
    pub fn function(&self, name: &str) -> Option<&IrFunction> {
        self.functions.iter().find(|function| function.name == name)
    }

    pub fn function_symbol(&self, function: &IrFunction) -> String {
        if function.name == "main" {
            return "main".to_string();
        }
        let module_path = self
            .modules
            .iter()
            .find(|module| module.id == function.module)
            .map(|module| module.path.as_slice())
            .unwrap_or(&[]);
        qualified_backend_symbol(module_path, &function.name)
    }
}

pub fn qualified_backend_symbol(module_path: &[String], name: &str) -> String {
    let module = if module_path.is_empty() {
        "root".to_string()
    } else {
        module_path
            .iter()
            .map(|segment| sanitize_symbol_part(segment))
            .collect::<Vec<_>>()
            .join("__")
    };
    format!("__mo_{module}__{}", sanitize_symbol_part(name))
}

fn sanitize_symbol_part(value: &str) -> String {
    let mut output = String::new();
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            output.push(ch);
        } else {
            output.push('_');
        }
    }
    if output.is_empty() {
        "_".to_string()
    } else {
        output
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrExternFunction {
    pub module: HirModuleId,
    pub name: String,
    pub abi: Option<String>,
    pub param_types: Vec<IrValueTy>,
    pub return_type: IrValueTy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrStruct {
    pub module: HirModuleId,
    pub name: String,
    pub fields: Vec<IrStructField>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrStructField {
    pub name: String,
    pub ty: IrValueTy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrEnum {
    pub module: HirModuleId,
    pub name: String,
    pub variants: Vec<IrEnumVariant>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrEnumVariant {
    pub name: String,
    pub tag: i64,
    pub payload_tys: Vec<IrValueTy>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IrValueTy {
    Unknown,
    Unit,
    Bool,
    Int(IrIntTy),
    Float64,
    String,
    BorrowedString,
    OwnedString,
    Boxed(Box<IrValueTy>),
    Vec(Box<IrValueTy>),
    Struct(String),
    Enum(String),
    Function {
        params: Vec<IrValueTy>,
        ret: Box<IrValueTy>,
        is_async: bool,
    },
}

pub fn is_string_ty(ty: &IrValueTy) -> bool {
    matches!(
        ty,
        IrValueTy::String | IrValueTy::BorrowedString | IrValueTy::OwnedString
    )
}

fn is_heap_owned_ty(ty: &IrValueTy) -> bool {
    if is_generic_placeholder_ty(ty) {
        return false;
    }
    matches!(
        ty,
        IrValueTy::OwnedString
            | IrValueTy::Boxed(_)
            | IrValueTy::Vec(_)
            | IrValueTy::Struct(_)
            | IrValueTy::Enum(_)
    )
}

fn is_shared_handle_ty(ty: &IrValueTy) -> bool {
    matches!(ty, IrValueTy::Struct(name) if is_shared_handle_name(name))
}

fn return_referenced_local_needs_temp(ty: &IrValueTy) -> bool {
    match ty {
        IrValueTy::OwnedString | IrValueTy::Boxed(_) | IrValueTy::Vec(_) => true,
        IrValueTy::Struct(name) => is_unique_resource_name(name),
        _ => false,
    }
}

fn is_generic_placeholder_ty(ty: &IrValueTy) -> bool {
    matches!(ty, IrValueTy::Struct(name) if name.len() == 1 && name.chars().all(|ch| ch.is_ascii_uppercase()))
}

fn is_byte_slice_ir_ty(ty: &IrValueTy) -> bool {
    matches!(ty, IrValueTy::Struct(name) if name == "ByteSlice" || name.ends_with("__ByteSlice"))
}

fn local_value_expr(local: &str, ty: Option<&IrValueTy>) -> IrValueExpr {
    match ty {
        Some(IrValueTy::String | IrValueTy::BorrowedString | IrValueTy::OwnedString) => {
            IrValueExpr::String(IrStringExpr::Local(local.to_string()))
        }
        Some(IrValueTy::Enum(_)) => IrValueExpr::Enum(IrEnumExpr::Local(local.to_string())),
        Some(IrValueTy::Int(_)) => IrValueExpr::Int(IrIntExpr::Local(local.to_string())),
        Some(IrValueTy::Float64) => IrValueExpr::Float(IrFloatExpr::Local(local.to_string())),
        Some(IrValueTy::Bool) => IrValueExpr::Bool(IrBoolExpr::Local(local.to_string())),
        Some(IrValueTy::Function { .. }) => {
            IrValueExpr::Function(IrFunctionExpr::Local(local.to_string()))
        }
        _ => IrValueExpr::Struct(IrStructExpr::Local(local.to_string())),
    }
}

fn enum_match_storage_drop(
    value: &IrEnumExpr,
    arms: &[IrEnumMatchArm],
    local_types: &HashMap<String, IrValueTy>,
) -> Option<(String, IrValueTy)> {
    if arms.iter().any(|arm| {
        arm.payload_tys
            .iter()
            .any(|ty| matches!(ty, IrValueTy::String) || is_heap_owned_ty(ty))
    }) {
        return None;
    }
    let IrEnumExpr::Local(local) = value else {
        return None;
    };
    let ty = local_types.get(local)?;
    is_heap_owned_ty(ty).then(|| (local.clone(), ty.clone()))
}

fn adjusted_string_return_ty(name: &str, ret: IrValueTy) -> IrValueTy {
    if is_known_buffer_finish_return(name) {
        return IrValueTy::String;
    }
    ret
}

fn is_known_buffer_finish_return(name: &str) -> bool {
    matches!(
        name,
        "finish"
            | "buffer__finish"
            | "string_builder_finish"
            | "buffer__string_builder_finish"
            | "byte_buffer_finish"
            | "buffer__byte_buffer_finish"
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IrIntTy {
    Int,
    Int8,
    Int16,
    Int32,
    Int64,
    UInt,
    UInt8,
    UInt16,
    UInt32,
    UInt64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrFunction {
    pub module: HirModuleId,
    pub name: String,
    pub is_async: bool,
    pub future_state: Option<IrFutureState>,
    pub params: Vec<String>,
    pub param_types: Vec<IrValueTy>,
    pub return_type: IrValueTy,
    pub blocks: Vec<IrBlock>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrFutureState {
    pub suspend_points: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrBlock {
    pub id: usize,
    pub instructions: Vec<IrInstruction>,
    pub terminator: IrTerminator,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IrInstruction {
    Let {
        local: String,
    },
    ConstInt {
        value: i64,
    },
    AssignInt {
        local: String,
        value: IrIntExpr,
    },
    AssignFloat {
        local: String,
        value: IrFloatExpr,
    },
    AssignBool {
        local: String,
        value: IrBoolExpr,
    },
    AssignString {
        local: String,
        value: IrStringExpr,
    },
    AssignStruct {
        local: String,
        value: IrStructExpr,
    },
    AssignEnum {
        local: String,
        value: IrEnumExpr,
    },
    AssignFunction {
        local: String,
        value: IrFunctionExpr,
    },
    AssignField {
        base: String,
        field: String,
        value: IrValueExpr,
    },
    AssignEnumMatch {
        local: String,
        ty: IrValueTy,
        value: IrEnumExpr,
        arms: Vec<IrEnumMatchArm>,
    },
    BindEnumPayload {
        local: String,
        value: IrEnumExpr,
        payload_index: usize,
        payload_tys: Vec<IrValueTy>,
        payload_ty: IrValueTy,
    },
    ReturnUnit,
    ReturnInt {
        value: IrIntExpr,
    },
    ReturnFloat {
        value: IrFloatExpr,
    },
    ReturnBool {
        value: IrBoolExpr,
    },
    ReturnString {
        value: IrStringExpr,
    },
    ReturnStruct {
        value: IrStructExpr,
    },
    ReturnEnum {
        value: IrEnumExpr,
    },
    ReturnEnumMatch {
        ty: IrValueTy,
        value: IrEnumExpr,
        arms: Vec<IrEnumMatchArm>,
        free_value_storage: bool,
    },
    ReturnFunction {
        value: IrFunctionExpr,
    },
    PrintString {
        value: String,
    },
    PrintStringExpr {
        value: IrStringExpr,
    },
    PrintInt {
        value: IrIntExpr,
    },
    Assert {
        condition: IrBoolExpr,
        message: String,
    },
    RawWrite {
        fd: IrIntExpr,
        text: IrStringExpr,
    },
    RawStore8 {
        ptr: IrIntExpr,
        offset: IrIntExpr,
        value: IrIntExpr,
    },
    RawStore64 {
        ptr: IrIntExpr,
        offset: IrIntExpr,
        value: IrIntExpr,
    },
    RawStringStore8 {
        value: IrStringExpr,
        offset: IrIntExpr,
        byte: IrIntExpr,
    },
    RawFree {
        ptr: IrIntExpr,
    },
    DropBoxStorage {
        local: String,
    },
    Expr {
        text: String,
    },
    Call {
        callee: String,
        args: Vec<IrValueExpr>,
    },
    IndirectCall {
        callee: IrFunctionExpr,
        args: Vec<IrValueExpr>,
    },
    Borrow {
        local: String,
        mutable: bool,
    },
    Suspend {
        point: usize,
    },
    FutureState {
        captures: Vec<String>,
    },
    BorrowAcrossSuspend {
        local: String,
    },
    Try {
        ok_block: usize,
        error_block: usize,
    },
    Drop {
        local: String,
        ty: IrValueTy,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IrIntExpr {
    Const(i64),
    Local(String),
    Field {
        base: String,
        field: String,
    },
    EnumTag(Box<IrEnumExpr>),
    Call {
        callee: String,
        args: Vec<IrValueExpr>,
    },
    IndirectCall {
        callee: IrFunctionExpr,
        args: Vec<IrValueExpr>,
    },
    StringLen(Box<IrStringExpr>),
    StringPtr(Box<IrStringExpr>),
    FunctionPtr(IrFunctionExpr),
    FloatToInt(Box<IrFloatExpr>),
    EnvLoad {
        offset: i32,
    },
    RawWrite {
        fd: Box<IrIntExpr>,
        text: Box<IrStringExpr>,
    },
    RawAlloc {
        size: Box<IrIntExpr>,
    },
    RawLoad8 {
        ptr: Box<IrIntExpr>,
        offset: Box<IrIntExpr>,
    },
    RawLoad64 {
        ptr: Box<IrIntExpr>,
        offset: Box<IrIntExpr>,
    },
    RawSetNonblocking {
        fd: Box<IrIntExpr>,
    },
    RawThreadSpawn {
        task: IrFunctionExpr,
        captures: Vec<IrValueExpr>,
    },
    RawThreadJoin {
        handle: Box<IrIntExpr>,
    },
    RawMemAllocCount,
    RawMemFreeCount,
    RawMemLiveBytes,
    RawMemHighWaterBytes,
    Binary {
        op: IrIntBinaryOp,
        left: Box<IrIntExpr>,
        right: Box<IrIntExpr>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IrStringExpr {
    Literal(String),
    Local(String),
    EnvLoad {
        offset: i32,
    },
    RawAlloc {
        size: Box<IrIntExpr>,
    },
    Field {
        base: String,
        field: String,
    },
    Concat {
        left: Box<IrStringExpr>,
        right: Box<IrStringExpr>,
    },
    IntToString(Box<IrIntExpr>),
    FromPtr(Box<IrIntExpr>),
    Call {
        callee: String,
        args: Vec<IrValueExpr>,
    },
    IndirectCall {
        callee: IrFunctionExpr,
        args: Vec<IrValueExpr>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IrStructExpr {
    Local(String),
    Field {
        base: String,
        field: String,
    },
    EnvLoad {
        offset: i32,
    },
    Construct {
        name: String,
        fields: Vec<IrStructFieldValue>,
    },
    Call {
        callee: String,
        args: Vec<IrValueExpr>,
    },
    IndirectCall {
        callee: IrFunctionExpr,
        args: Vec<IrValueExpr>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IrFunctionExpr {
    Local(String),
    Named(String),
    Field {
        base: String,
        field: String,
    },
    Call {
        callee: String,
        args: Vec<IrValueExpr>,
    },
    FromPtr(Box<IrIntExpr>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrStructFieldValue {
    pub name: String,
    pub value: IrValueExpr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IrValueExpr {
    Int(IrIntExpr),
    Float(IrFloatExpr),
    Bool(IrBoolExpr),
    String(IrStringExpr),
    Struct(IrStructExpr),
    Enum(IrEnumExpr),
    Function(IrFunctionExpr),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IrEnumExpr {
    Local(String),
    EnvLoad {
        offset: i32,
    },
    Call {
        callee: String,
        args: Vec<IrValueExpr>,
    },
    IndirectCall {
        callee: IrFunctionExpr,
        args: Vec<IrValueExpr>,
    },
    Construct {
        enum_name: String,
        variant: String,
        tag: i64,
        payloads: Vec<IrValueExpr>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrEnumMatchArm {
    pub variant: String,
    pub tag: i64,
    pub bindings: Vec<String>,
    pub payload_tys: Vec<IrValueTy>,
    pub body: IrValueExpr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IrBoolExpr {
    Const(bool),
    Local(String),
    Field {
        base: String,
        field: String,
    },
    EnvLoad {
        offset: i32,
    },
    Call {
        callee: String,
        args: Vec<IrValueExpr>,
    },
    Not(Box<IrBoolExpr>),
    And(Box<IrBoolExpr>, Box<IrBoolExpr>),
    Or(Box<IrBoolExpr>, Box<IrBoolExpr>),
    Compare {
        op: IrCompareOp,
        left: Box<IrIntExpr>,
        right: Box<IrIntExpr>,
    },
    FloatCompare {
        op: IrCompareOp,
        left: Box<IrFloatExpr>,
        right: Box<IrFloatExpr>,
    },
    BoolCompare {
        op: IrCompareOp,
        left: Box<IrBoolExpr>,
        right: Box<IrBoolExpr>,
    },
    StringCompare {
        op: IrCompareOp,
        left: Box<IrStringExpr>,
        right: Box<IrStringExpr>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IrCompareOp {
    Eq,
    NotEq,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IrIntBinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IrFloatExpr {
    Const(String),
    Local(String),
    Field {
        base: String,
        field: String,
    },
    EnvLoad {
        offset: i32,
    },
    Call {
        callee: String,
        args: Vec<IrValueExpr>,
    },
    IndirectCall {
        callee: IrFunctionExpr,
        args: Vec<IrValueExpr>,
    },
    IntToFloat(Box<IrIntExpr>),
    Binary {
        op: IrFloatBinaryOp,
        left: Box<IrFloatExpr>,
        right: Box<IrFloatExpr>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IrFloatBinaryOp {
    Add,
    Sub,
    Mul,
    Div,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IrTerminator {
    Return,
    Jump {
        target: usize,
    },
    Branch {
        condition: Option<IrBoolExpr>,
        then_block: usize,
        else_block: usize,
    },
    Switch {
        value: IrEnumExpr,
        arms: Vec<IrSwitchArm>,
        fallback: usize,
    },
    Unreachable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrSwitchArm {
    pub tag: i64,
    pub target: usize,
}

pub fn lower_to_ir(program: &HirProgram, drops: &DropReport) -> IrProgram {
    let enum_names = program
        .enums
        .iter()
        .map(|item| item.name.clone())
        .collect::<HashSet<_>>();
    let generic_enum_names = program
        .enums
        .iter()
        .filter(|item| {
            item.generics
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty())
        })
        .map(|item| item.name.clone())
        .collect::<HashSet<_>>();
    let generic_struct_names = program
        .structs
        .iter()
        .filter(|item| {
            item.generics
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty())
                && !is_builtin_generic_resource_name(&item.name)
        })
        .map(|item| item.name.clone())
        .collect::<HashSet<_>>();
    let mut structs = program
        .structs
        .iter()
        .map(|item| IrStruct {
            module: item.module,
            name: item.name.clone(),
            fields: item
                .fields
                .iter()
                .map(|field| IrStructField {
                    name: field.name.clone(),
                    ty: ir_ty_from_type(&field.ty_expr, &enum_names, &generic_struct_names),
                })
                .collect(),
        })
        .collect::<Vec<_>>();
    let mut enums = program
        .enums
        .iter()
        .map(|item| IrEnum {
            module: item.module,
            name: item.name.clone(),
            variants: item
                .variants
                .iter()
                .enumerate()
                .map(|(index, variant)| IrEnumVariant {
                    name: variant.name.clone(),
                    tag: index as i64,
                    payload_tys: variant
                        .payload
                        .as_deref()
                        .map(|payload| {
                            split_top_level_csv(payload)
                                .into_iter()
                                .map(|ty| ir_ty_from_payload_text(ty, &enum_names))
                                .collect()
                        })
                        .unwrap_or_default(),
                })
                .collect(),
        })
        .collect::<Vec<_>>();
    enums.extend(specialized_enums_from_program(
        program,
        &enum_names,
        &generic_enum_names,
        &generic_struct_names,
    ));
    let function_sigs = program
        .functions
        .iter()
        .map(|function| {
            let ret = function
                .return_type
                .as_ref()
                .map(|ty| ir_return_ty_from_type(ty, &enum_names, &generic_struct_names))
                .unwrap_or(IrValueTy::Unit);
            (
                function.name.clone(),
                FunctionSig {
                    params: function
                        .params
                        .iter()
                        .map(|param| {
                            param
                                .ty_expr
                                .as_ref()
                                .map(|ty| {
                                    ir_param_ty_from_type(ty, &enum_names, &generic_struct_names)
                                })
                                .unwrap_or(IrValueTy::Unknown)
                        })
                        .collect(),
                    ret: adjusted_string_return_ty(&function.name, ret),
                },
            )
        })
        .collect::<HashMap<_, _>>();
    let mut function_sigs = function_sigs;
    for function in &program.extern_functions {
        let ret = function
            .return_type
            .as_ref()
            .map(|ty| ir_return_ty_from_type(ty, &enum_names, &generic_struct_names))
            .unwrap_or(IrValueTy::Unit);
        function_sigs.insert(
            function.name.clone(),
            FunctionSig {
                params: function
                    .params
                    .iter()
                    .map(|param| {
                        param
                            .ty_expr
                            .as_ref()
                            .map(|ty| ir_param_ty_from_type(ty, &enum_names, &generic_struct_names))
                            .unwrap_or(IrValueTy::Unknown)
                    })
                    .collect(),
                ret: adjusted_string_return_ty(&function.name, ret),
            },
        );
    }
    structs.extend(specialized_structs_from_program(
        program,
        &enum_names,
        &generic_struct_names,
        &function_sigs,
    ));
    let struct_fields = structs
        .iter()
        .map(|item| {
            (
                item.name.clone(),
                item.fields
                    .iter()
                    .map(|field| (field.name.clone(), field.ty.clone()))
                    .collect::<HashMap<_, _>>(),
            )
        })
        .collect::<HashMap<_, _>>();
    let drop_impls = drop_impls_from_program(program);
    let enum_variants = enums
        .iter()
        .flat_map(|item| {
            item.variants.iter().map(|variant| {
                (
                    variant.name.clone(),
                    EnumVariantSig {
                        enum_name: item.name.clone(),
                        tag: variant.tag,
                        payload_tys: variant.payload_tys.clone(),
                    },
                )
            })
        })
        .collect::<HashMap<_, _>>();
    let enum_variants_by_enum = enums
        .iter()
        .map(|item| {
            (
                item.name.clone(),
                item.variants
                    .iter()
                    .map(|variant| {
                        (
                            variant.name.clone(),
                            EnumVariantSig {
                                enum_name: item.name.clone(),
                                tag: variant.tag,
                                payload_tys: variant.payload_tys.clone(),
                            },
                        )
                    })
                    .collect::<HashMap<_, _>>(),
            )
        })
        .collect::<HashMap<_, _>>();
    let mut functions = Vec::new();
    for function in &program.functions {
        let lowered = lower_function(
            function,
            drops,
            &function_sigs,
            &struct_fields,
            &enum_variants,
            &enum_variants_by_enum,
            &enum_names,
            &generic_struct_names,
        );
        functions.push(lowered.function);
        functions.extend(lowered.generated);
    }

    IrProgram {
        modules: program
            .modules
            .iter()
            .map(|module| IrModule {
                id: module.id,
                path: module.path.clone(),
            })
            .collect(),
        extern_functions: program
            .extern_functions
            .iter()
            .map(|function| IrExternFunction {
                module: function.module,
                name: function.name.clone(),
                abi: function.abi.clone(),
                param_types: function
                    .params
                    .iter()
                    .map(|param| {
                        param
                            .ty_expr
                            .as_ref()
                            .map(|ty| ir_param_ty_from_type(ty, &enum_names, &generic_struct_names))
                            .unwrap_or(IrValueTy::Unknown)
                    })
                    .collect(),
                return_type: function
                    .return_type
                    .as_ref()
                    .map(|ty| ir_return_ty_from_type(ty, &enum_names, &generic_struct_names))
                    .unwrap_or(IrValueTy::Unit),
            })
            .collect(),
        structs,
        enums,
        functions,
        drop_impls,
    }
}

fn drop_impls_from_program(program: &HirProgram) -> HashMap<String, String> {
    program
        .impls
        .iter()
        .filter(|implementation| implementation.interface.as_deref() == Some("Drop"))
        .filter_map(|implementation| {
            let method = implementation
                .methods
                .iter()
                .find(|method| method.name == "drop")?;
            Some((implementation.target.clone(), method.name.clone()))
        })
        .collect()
}

fn is_by_value_string_type(ty: &TypeExpr) -> bool {
    matches!(ty, TypeExpr::Path(path) if path.first().is_some_and(|name| name == "String"))
}

fn ir_param_ty_from_type(
    ty: &TypeExpr,
    enum_names: &HashSet<String>,
    generic_struct_names: &HashSet<String>,
) -> IrValueTy {
    if is_by_value_string_type(ty) {
        IrValueTy::OwnedString
    } else {
        ir_ty_from_type(ty, enum_names, generic_struct_names)
    }
}

fn ir_return_ty_from_type(
    ty: &TypeExpr,
    enum_names: &HashSet<String>,
    generic_struct_names: &HashSet<String>,
) -> IrValueTy {
    if is_by_value_string_type(ty) {
        IrValueTy::OwnedString
    } else {
        ir_ty_from_type(ty, enum_names, generic_struct_names)
    }
}

fn ir_local_ty_from_type(
    ty: &TypeExpr,
    enum_names: &HashSet<String>,
    generic_struct_names: &HashSet<String>,
) -> IrValueTy {
    if is_by_value_string_type(ty) {
        IrValueTy::OwnedString
    } else {
        ir_ty_from_type(ty, enum_names, generic_struct_names)
    }
}

fn specialized_structs_from_program(
    program: &HirProgram,
    enum_names: &HashSet<String>,
    generic_struct_names: &HashSet<String>,
    function_sigs: &HashMap<String, FunctionSig>,
) -> Vec<IrStruct> {
    let generic_structs = program
        .structs
        .iter()
        .filter(|item| generic_struct_names.contains(&item.name))
        .map(|item| (item.name.clone(), item))
        .collect::<HashMap<_, _>>();
    let mut specialized = Vec::new();
    let mut seen = HashSet::new();

    for function in &program.functions {
        for param in &function.params {
            if let Some(ty) = &param.ty_expr {
                collect_specialized_struct_ty(
                    ty,
                    &generic_structs,
                    enum_names,
                    generic_struct_names,
                    &mut seen,
                    &mut specialized,
                );
            }
        }
        if let Some(ty) = &function.return_type {
            collect_specialized_struct_ty(
                ty,
                &generic_structs,
                enum_names,
                generic_struct_names,
                &mut seen,
                &mut specialized,
            );
        }
        collect_specialized_struct_block(
            &function.body,
            &generic_structs,
            enum_names,
            generic_struct_names,
            function_sigs,
            &mut seen,
            &mut specialized,
        );
    }
    for test in &program.tests {
        collect_specialized_struct_block(
            &test.body,
            &generic_structs,
            enum_names,
            generic_struct_names,
            function_sigs,
            &mut seen,
            &mut specialized,
        );
    }

    specialized
}

fn specialized_enums_from_program(
    program: &HirProgram,
    enum_names: &HashSet<String>,
    generic_enum_names: &HashSet<String>,
    generic_struct_names: &HashSet<String>,
) -> Vec<IrEnum> {
    let generic_enums = program
        .enums
        .iter()
        .filter(|item| generic_enum_names.contains(&item.name))
        .map(|item| (item.name.clone(), item))
        .collect::<HashMap<_, _>>();
    let mut specialized = Vec::new();
    let mut seen = HashSet::new();

    for function in &program.functions {
        for param in &function.params {
            if let Some(ty) = &param.ty_expr {
                collect_specialized_enum_ty(
                    ty,
                    &generic_enums,
                    enum_names,
                    generic_enum_names,
                    generic_struct_names,
                    &mut seen,
                    &mut specialized,
                );
            }
        }
        if let Some(ty) = &function.return_type {
            collect_specialized_enum_ty(
                ty,
                &generic_enums,
                enum_names,
                generic_enum_names,
                generic_struct_names,
                &mut seen,
                &mut specialized,
            );
        }
        collect_specialized_enum_block(
            &function.body,
            &generic_enums,
            enum_names,
            generic_enum_names,
            generic_struct_names,
            &mut seen,
            &mut specialized,
        );
    }
    for test in &program.tests {
        collect_specialized_enum_block(
            &test.body,
            &generic_enums,
            enum_names,
            generic_enum_names,
            generic_struct_names,
            &mut seen,
            &mut specialized,
        );
    }

    specialized
}

fn collect_specialized_enum_block(
    block: &Block,
    generic_enums: &HashMap<String, &crate::hir::HirEnum>,
    enum_names: &HashSet<String>,
    generic_enum_names: &HashSet<String>,
    generic_struct_names: &HashSet<String>,
    seen: &mut HashSet<String>,
    specialized: &mut Vec<IrEnum>,
) {
    for stmt in &block.statements {
        match &stmt.data {
            StmtData::Let(stmt) => {
                if let Some(ty) = &stmt.ty_expr {
                    collect_specialized_enum_ty(
                        ty,
                        generic_enums,
                        enum_names,
                        generic_enum_names,
                        generic_struct_names,
                        seen,
                        specialized,
                    );
                }
            }
            StmtData::If(control) | StmtData::While(control) => collect_specialized_enum_block(
                &control.body,
                generic_enums,
                enum_names,
                generic_enum_names,
                generic_struct_names,
                seen,
                specialized,
            ),
            StmtData::For(stmt) => collect_specialized_enum_block(
                &stmt.body,
                generic_enums,
                enum_names,
                generic_enum_names,
                generic_struct_names,
                seen,
                specialized,
            ),
            StmtData::Loop(block) | StmtData::Unsafe(block) => collect_specialized_enum_block(
                block,
                generic_enums,
                enum_names,
                generic_enum_names,
                generic_struct_names,
                seen,
                specialized,
            ),
            _ => {}
        }
    }
}

fn collect_specialized_enum_ty(
    ty: &TypeExpr,
    generic_enums: &HashMap<String, &crate::hir::HirEnum>,
    enum_names: &HashSet<String>,
    generic_enum_names: &HashSet<String>,
    generic_struct_names: &HashSet<String>,
    seen: &mut HashSet<String>,
    specialized: &mut Vec<IrEnum>,
) {
    match ty {
        TypeExpr::Generic { base, args } => {
            for arg in args {
                collect_specialized_enum_ty(
                    arg,
                    generic_enums,
                    enum_names,
                    generic_enum_names,
                    generic_struct_names,
                    seen,
                    specialized,
                );
            }
            let Some(base_name) = type_base_name(base) else {
                return;
            };
            let Some(item) = generic_enums.get(&base_name) else {
                return;
            };
            let name = generic_instance_name(base, args, enum_names, generic_struct_names);
            if !seen.insert(name.clone()) {
                return;
            }
            let params = parse_generic_params(item.generics.as_deref());
            let substitutions = params
                .into_iter()
                .zip(
                    args.iter()
                        .map(|arg| ir_ty_from_type(arg, enum_names, generic_struct_names)),
                )
                .collect::<HashMap<_, _>>();
            specialized.push(IrEnum {
                module: item.module,
                name,
                variants: item
                    .variants
                    .iter()
                    .enumerate()
                    .map(|(index, variant)| IrEnumVariant {
                        name: variant.name.clone(),
                        tag: index as i64,
                        payload_tys: variant
                            .payload
                            .as_deref()
                            .map(|payload| {
                                split_top_level_csv(payload)
                                    .into_iter()
                                    .map(|ty| {
                                        ir_ty_from_payload_text_with_substitutions(
                                            ty,
                                            enum_names,
                                            &substitutions,
                                        )
                                    })
                                    .collect()
                            })
                            .unwrap_or_default(),
                    })
                    .collect(),
            });
        }
        TypeExpr::Fn {
            params,
            return_type,
            ..
        } => {
            for param in params {
                collect_specialized_enum_ty(
                    param,
                    generic_enums,
                    enum_names,
                    generic_enum_names,
                    generic_struct_names,
                    seen,
                    specialized,
                );
            }
            collect_specialized_enum_ty(
                return_type,
                generic_enums,
                enum_names,
                generic_enum_names,
                generic_struct_names,
                seen,
                specialized,
            );
        }
        TypeExpr::Ref { inner, .. } | TypeExpr::Impl(inner) | TypeExpr::Mut(inner) => {
            collect_specialized_enum_ty(
                inner,
                generic_enums,
                enum_names,
                generic_enum_names,
                generic_struct_names,
                seen,
                specialized,
            );
        }
        TypeExpr::Tuple(items) => {
            for item in items {
                collect_specialized_enum_ty(
                    item,
                    generic_enums,
                    enum_names,
                    generic_enum_names,
                    generic_struct_names,
                    seen,
                    specialized,
                );
            }
        }
        _ => {}
    }
}

fn is_builtin_generic_resource_name(name: &str) -> bool {
    let base = name.rsplit("__").next().unwrap_or(name);
    matches!(base, "Box" | "Channel" | "Map" | "Shared" | "Vec")
}

fn collect_specialized_struct_block(
    block: &Block,
    generic_structs: &HashMap<String, &crate::hir::HirStruct>,
    enum_names: &HashSet<String>,
    generic_struct_names: &HashSet<String>,
    function_sigs: &HashMap<String, FunctionSig>,
    seen: &mut HashSet<String>,
    specialized: &mut Vec<IrStruct>,
) {
    for stmt in &block.statements {
        match &stmt.data {
            StmtData::Let(stmt) => {
                if let Some(ty) = &stmt.ty_expr {
                    collect_specialized_struct_ty(
                        ty,
                        generic_structs,
                        enum_names,
                        generic_struct_names,
                        seen,
                        specialized,
                    );
                } else if let Some(Expr::Struct(expr)) = &stmt.value {
                    collect_specialized_struct_literal(
                        expr,
                        generic_structs,
                        enum_names,
                        generic_struct_names,
                        function_sigs,
                        seen,
                        specialized,
                    );
                }
            }
            StmtData::If(control) | StmtData::While(control) => collect_specialized_struct_block(
                &control.body,
                generic_structs,
                enum_names,
                generic_struct_names,
                function_sigs,
                seen,
                specialized,
            ),
            StmtData::For(stmt) => collect_specialized_struct_block(
                &stmt.body,
                generic_structs,
                enum_names,
                generic_struct_names,
                function_sigs,
                seen,
                specialized,
            ),
            StmtData::Loop(block) | StmtData::Unsafe(block) => collect_specialized_struct_block(
                block,
                generic_structs,
                enum_names,
                generic_struct_names,
                function_sigs,
                seen,
                specialized,
            ),
            _ => {}
        }
    }
}

fn collect_specialized_struct_ty(
    ty: &TypeExpr,
    generic_structs: &HashMap<String, &crate::hir::HirStruct>,
    enum_names: &HashSet<String>,
    generic_struct_names: &HashSet<String>,
    seen: &mut HashSet<String>,
    specialized: &mut Vec<IrStruct>,
) {
    match ty {
        TypeExpr::Generic { base, args } => {
            for arg in args {
                collect_specialized_struct_ty(
                    arg,
                    generic_structs,
                    enum_names,
                    generic_struct_names,
                    seen,
                    specialized,
                );
            }
            let Some(base_name) = type_base_name(base) else {
                return;
            };
            let Some(item) = generic_structs.get(&base_name) else {
                return;
            };
            let name = generic_instance_name(base, args, enum_names, generic_struct_names);
            if !seen.insert(name.clone()) {
                return;
            }
            let params = parse_generic_params(item.generics.as_deref());
            let substitutions = params
                .into_iter()
                .zip(
                    args.iter()
                        .map(|arg| ir_ty_from_type(arg, enum_names, generic_struct_names)),
                )
                .collect::<HashMap<_, _>>();
            specialized.push(IrStruct {
                module: item.module,
                name,
                fields: item
                    .fields
                    .iter()
                    .map(|field| IrStructField {
                        name: field.name.clone(),
                        ty: ir_ty_from_type_with_substitutions(
                            &field.ty_expr,
                            enum_names,
                            generic_struct_names,
                            &substitutions,
                        ),
                    })
                    .collect(),
            });
        }
        TypeExpr::Fn {
            params,
            return_type,
            ..
        } => {
            for param in params {
                collect_specialized_struct_ty(
                    param,
                    generic_structs,
                    enum_names,
                    generic_struct_names,
                    seen,
                    specialized,
                );
            }
            collect_specialized_struct_ty(
                return_type,
                generic_structs,
                enum_names,
                generic_struct_names,
                seen,
                specialized,
            );
        }
        TypeExpr::Ref { inner, .. } | TypeExpr::Impl(inner) | TypeExpr::Mut(inner) => {
            collect_specialized_struct_ty(
                inner,
                generic_structs,
                enum_names,
                generic_struct_names,
                seen,
                specialized,
            );
        }
        TypeExpr::Tuple(items) => {
            for item in items {
                collect_specialized_struct_ty(
                    item,
                    generic_structs,
                    enum_names,
                    generic_struct_names,
                    seen,
                    specialized,
                );
            }
        }
        _ => {}
    }
}

fn collect_specialized_struct_literal(
    expr: &StructExpr,
    generic_structs: &HashMap<String, &crate::hir::HirStruct>,
    enum_names: &HashSet<String>,
    generic_struct_names: &HashSet<String>,
    function_sigs: &HashMap<String, FunctionSig>,
    seen: &mut HashSet<String>,
    specialized: &mut Vec<IrStruct>,
) {
    let Some(item) = generic_structs.get(&expr.name) else {
        return;
    };
    let params = parse_generic_params(item.generics.as_deref());
    if params.is_empty() {
        return;
    }
    let mut substitutions = HashMap::new();
    for field in &expr.fields {
        let Some(value) = &field.value else {
            continue;
        };
        let Some(field_def) = item
            .fields
            .iter()
            .find(|candidate| candidate.name == field.name)
        else {
            continue;
        };
        let TypeExpr::Path(path) = &field_def.ty_expr else {
            continue;
        };
        let Some(generic) = path
            .first()
            .filter(|name| params.iter().any(|param| param == *name))
        else {
            continue;
        };
        if let Some(ty) =
            infer_specialized_expr_ty(value, enum_names, generic_struct_names, function_sigs)
        {
            substitutions.insert(generic.clone(), ty);
        }
    }
    if substitutions.is_empty() {
        return;
    }
    let args = params
        .iter()
        .map(|param| {
            substitutions
                .get(param)
                .cloned()
                .unwrap_or(IrValueTy::Unknown)
        })
        .collect::<Vec<_>>();
    let name = format!(
        "{}<{}>",
        expr.name,
        args.iter().map(ir_ty_name).collect::<Vec<_>>().join(",")
    );
    if !seen.insert(name.clone()) {
        return;
    }
    specialized.push(IrStruct {
        module: item.module,
        name,
        fields: item
            .fields
            .iter()
            .map(|field| IrStructField {
                name: field.name.clone(),
                ty: ir_ty_from_type_with_substitutions(
                    &field.ty_expr,
                    enum_names,
                    generic_struct_names,
                    &substitutions,
                ),
            })
            .collect(),
    });
}

fn infer_specialized_expr_ty(
    expr: &Expr,
    enum_names: &HashSet<String>,
    generic_struct_names: &HashSet<String>,
    function_sigs: &HashMap<String, FunctionSig>,
) -> Option<IrValueTy> {
    match expr {
        Expr::Literal(Literal::String(_)) => Some(IrValueTy::String),
        Expr::Literal(Literal::Bool(_)) => Some(IrValueTy::Bool),
        Expr::Literal(Literal::Int(_)) => Some(IrValueTy::Int(IrIntTy::Int)),
        Expr::Literal(Literal::Float(_)) => Some(IrValueTy::Float64),
        Expr::Call(call) => {
            let callee = call_callee_name(&call.callee)?;
            function_sigs.get(callee).map(|sig| sig.ret.clone())
        }
        Expr::Await(expr) | Expr::Try(expr) => {
            infer_specialized_expr_ty(expr, enum_names, generic_struct_names, function_sigs)
        }
        Expr::Unary(expr) => {
            infer_specialized_expr_ty(&expr.expr, enum_names, generic_struct_names, function_sigs)
        }
        Expr::Mut(expr) => {
            infer_specialized_expr_ty(expr, enum_names, generic_struct_names, function_sigs)
        }
        Expr::Block(block) => block
            .statements
            .iter()
            .rev()
            .find_map(|stmt| match &stmt.data {
                StmtData::Return(Some(expr)) | StmtData::Expr(expr) => {
                    infer_specialized_expr_ty(expr, enum_names, generic_struct_names, function_sigs)
                }
                _ => None,
            }),
        _ => None,
    }
}

#[derive(Debug, Clone)]
struct FunctionSig {
    params: Vec<IrValueTy>,
    ret: IrValueTy,
}

fn ir_fn_ty_from_sig(sig: &FunctionSig) -> IrValueTy {
    IrValueTy::Function {
        params: sig.params.clone(),
        ret: Box::new(sig.ret.clone()),
        is_async: false,
    }
}

#[derive(Debug, Clone)]
struct EnumVariantSig {
    enum_name: String,
    tag: i64,
    payload_tys: Vec<IrValueTy>,
}

#[derive(Debug, Clone)]
struct LoweredFunction {
    function: IrFunction,
    generated: Vec<IrFunction>,
}

fn lower_function(
    function: &HirFunction,
    drops: &DropReport,
    function_sigs: &HashMap<String, FunctionSig>,
    struct_fields: &HashMap<String, HashMap<String, IrValueTy>>,
    enum_variants: &HashMap<String, EnumVariantSig>,
    enum_variants_by_enum: &HashMap<String, HashMap<String, EnumVariantSig>>,
    enum_names: &HashSet<String>,
    generic_struct_names: &HashSet<String>,
) -> LoweredFunction {
    let param_types = function
        .params
        .iter()
        .map(|param| {
            param
                .ty_expr
                .as_ref()
                .map(|ty| ir_param_ty_from_type(ty, enum_names, generic_struct_names))
                .unwrap_or(IrValueTy::Unknown)
        })
        .collect::<Vec<_>>();
    let consumed_locals = ownership_transfer_param_indexes(&function.name)
        .iter()
        .filter_map(|index| function.params.get(*index))
        .map(|param| normalize_param_name(&param.name))
        .collect::<HashSet<_>>();
    let mut lowerer = FunctionLowerer {
        blocks: vec![IrBlock {
            id: 0,
            instructions: Vec::new(),
            terminator: IrTerminator::Return,
        }],
        current: 0,
        loop_stack: Vec::new(),
        locals: function
            .params
            .iter()
            .map(|param| normalize_param_name(&param.name))
            .collect(),
        local_types: function
            .params
            .iter()
            .zip(param_types.iter())
            .map(|(param, ty)| (normalize_param_name(&param.name), ty.clone()))
            .collect(),
        channel_local_type_args: function
            .params
            .iter()
            .filter_map(|param| {
                let name = normalize_param_name(&param.name);
                let type_arg = param
                    .ty_expr
                    .as_ref()
                    .and_then(channel_type_arg_from_type_expr)?;
                Some((name, type_arg))
            })
            .collect(),
        box_local_type_args: function
            .params
            .iter()
            .filter_map(|param| {
                let name = normalize_param_name(&param.name);
                let type_arg = param
                    .ty_expr
                    .as_ref()
                    .and_then(box_type_arg_from_type_expr)?;
                Some((name, type_arg))
            })
            .collect(),
        map_local_type_args: function
            .params
            .iter()
            .filter_map(|param| {
                let name = normalize_param_name(&param.name);
                let type_arg = param
                    .ty_expr
                    .as_ref()
                    .and_then(map_type_arg_from_type_expr)?;
                Some((name, type_arg))
            })
            .collect(),
        vec_local_type_args: function
            .params
            .iter()
            .filter_map(|param| {
                let name = normalize_param_name(&param.name);
                let type_arg = param
                    .ty_expr
                    .as_ref()
                    .and_then(vec_type_arg_from_type_expr)?;
                Some((name, type_arg))
            })
            .collect(),
        function_sigs,
        struct_fields,
        enum_variants,
        enum_variants_by_enum,
        enum_names,
        generic_struct_names,
        return_type: function
            .return_type
            .as_ref()
            .map(|ty| ir_return_ty_from_type(ty, enum_names, generic_struct_names))
            .unwrap_or(IrValueTy::Unit),
        live_borrows: Vec::new(),
        suspend_points: 0,
        module: function.module,
        function_name: function.name.clone(),
        closure_index: 0,
        generated_functions: Vec::new(),
        return_temp_index: 0,
        assignment_temp_index: 0,
        consumed_locals,
        return_drop_locals: drops
            .function_drops
            .get(&function.name)
            .cloned()
            .unwrap_or_default(),
        scope_stack: Vec::new(),
    };
    lowerer.lower_block(&function.body);
    lowerer.append_drops(
        drops
            .function_drops
            .get(&function.name)
            .map(Vec::as_slice)
            .unwrap_or(&[]),
    );

    let function = IrFunction {
        module: function.module,
        name: function.name.clone(),
        is_async: function.is_async,
        future_state: function.is_async.then_some(IrFutureState {
            suspend_points: lowerer.suspend_points,
        }),
        params: function
            .params
            .iter()
            .map(|param| normalize_param_name(&param.name))
            .collect(),
        param_types,
        return_type: function
            .return_type
            .as_ref()
            .map(|ty| ir_return_ty_from_type(ty, enum_names, generic_struct_names))
            .unwrap_or(IrValueTy::Unit),
        blocks: lowerer.blocks,
    };
    LoweredFunction {
        function,
        generated: lowerer.generated_functions,
    }
}

struct FunctionLowerer<'a> {
    blocks: Vec<IrBlock>,
    current: usize,
    loop_stack: Vec<LoopTargets>,
    locals: Vec<String>,
    local_types: HashMap<String, IrValueTy>,
    channel_local_type_args: HashMap<String, String>,
    box_local_type_args: HashMap<String, String>,
    map_local_type_args: HashMap<String, String>,
    vec_local_type_args: HashMap<String, String>,
    function_sigs: &'a HashMap<String, FunctionSig>,
    struct_fields: &'a HashMap<String, HashMap<String, IrValueTy>>,
    enum_variants: &'a HashMap<String, EnumVariantSig>,
    enum_variants_by_enum: &'a HashMap<String, HashMap<String, EnumVariantSig>>,
    enum_names: &'a HashSet<String>,
    generic_struct_names: &'a HashSet<String>,
    return_type: IrValueTy,
    live_borrows: Vec<String>,
    suspend_points: usize,
    module: HirModuleId,
    function_name: String,
    closure_index: usize,
    generated_functions: Vec<IrFunction>,
    return_temp_index: usize,
    assignment_temp_index: usize,
    consumed_locals: HashSet<String>,
    return_drop_locals: Vec<String>,
    scope_stack: Vec<Vec<String>>,
}

#[derive(Debug, Clone, Copy)]
struct LoopTargets {
    continue_block: usize,
    break_block: usize,
}

impl<'a> FunctionLowerer<'a> {
    fn lower_block(&mut self, block: &Block) {
        self.scope_stack.push(Vec::new());
        for stmt in &block.statements {
            self.lower_stmt(stmt);
        }
        if !self.current_block_returns() {
            self.append_scope_exit_drops(&HashSet::new());
        }
        self.scope_stack.pop();
    }

    fn lower_stmt(&mut self, stmt: &Stmt) {
        match &stmt.data {
            StmtData::Let(stmt) => {
                let declared_channel_type_arg = stmt
                    .ty_expr
                    .as_ref()
                    .and_then(channel_type_arg_from_type_expr);
                let declared_box_type_arg =
                    stmt.ty_expr.as_ref().and_then(box_type_arg_from_type_expr);
                let declared_map_type_arg =
                    stmt.ty_expr.as_ref().and_then(map_type_arg_from_type_expr);
                let declared_vec_type_arg =
                    stmt.ty_expr.as_ref().and_then(vec_type_arg_from_type_expr);
                let declared_ty = stmt.ty_expr.as_ref().map(|ty| {
                    ir_local_ty_from_type(ty, self.enum_names, self.generic_struct_names)
                });
                let inferred_initializer_ty = stmt.value.as_ref().and_then(|value| {
                    if let Expr::Try(expr) = value {
                        self.infer_try_unwrap_ty(expr)
                    } else {
                        self.infer_expr_ty(Some(value))
                    }
                });
                if let Some(value) = &stmt.value {
                    if matches!(value, Expr::Try(expr) if {
                        self.lower_try_assignment(&stmt.name, expr);
                        true
                    }) {
                    } else if matches!(value, Expr::Block(block) if {
                        self.lower_assignment_block_value(
                            &stmt.name,
                            block,
                            declared_ty.as_ref().or(inferred_initializer_ty.as_ref()),
                        );
                        true
                    }) {
                    } else if matches!(value, Expr::If(if_expr) if self.lower_if_assignment(&stmt.name, if_expr, declared_ty.as_ref().or(inferred_initializer_ty.as_ref())))
                    {
                    } else if let Some(value) = self
                        .lower_function_assignment(value)
                        .or_else(|| self.lower_function_expr(value))
                    {
                        self.push(IrInstruction::AssignFunction {
                            local: stmt.name.clone(),
                            value,
                        });
                    } else if let Some(value) =
                        self.lower_enum_expr_with_expected(value, declared_ty.as_ref())
                    {
                        self.push(IrInstruction::AssignEnum {
                            local: stmt.name.clone(),
                            value,
                        });
                    } else if let Some(value) = self.lower_struct_expr_with_channel_type_arg(
                        value,
                        declared_channel_type_arg.as_deref(),
                        declared_ty.as_ref().or(inferred_initializer_ty.as_ref()),
                    ) {
                        self.push(IrInstruction::AssignStruct {
                            local: stmt.name.clone(),
                            value,
                        });
                    } else if let Some(value) = self.lower_string_expr_with_channel_type_arg(
                        value,
                        declared_channel_type_arg.as_deref(),
                    ) {
                        let box_take_string_arg =
                            consumed_box_take_string_arg_from_ir_string(&value);
                        self.push(IrInstruction::AssignString {
                            local: stmt.name.clone(),
                            value,
                        });
                        if let Some(local) = box_take_string_arg {
                            self.consumed_locals.insert(local.clone());
                            self.push(IrInstruction::DropBoxStorage { local });
                        }
                    } else if let Some((ty, value, arms)) = self.lower_enum_match_value(
                        value,
                        stmt.ty_expr
                            .as_ref()
                            .map(|ty| {
                                ir_local_ty_from_type(
                                    ty,
                                    self.enum_names,
                                    self.generic_struct_names,
                                )
                            })
                            .as_ref(),
                    ) {
                        self.push(IrInstruction::AssignEnumMatch {
                            local: stmt.name.clone(),
                            ty,
                            value,
                            arms,
                        });
                    } else if declared_ty.as_ref().or(inferred_initializer_ty.as_ref())
                        == Some(&IrValueTy::Float64)
                    {
                        if let Some(value) = self.lower_float_expr(value) {
                            self.push(IrInstruction::AssignFloat {
                                local: stmt.name.clone(),
                                value,
                            });
                        } else {
                            self.lower_expr(value);
                        }
                    } else if let Some(value) = self.lower_int_expr(value) {
                        if let Some(local) = consumed_cleanup_arg_from_ir_int(&value) {
                            self.consumed_locals.insert(local);
                        }
                        self.push(IrInstruction::AssignInt {
                            local: stmt.name.clone(),
                            value,
                        });
                    } else if let Some(value) = self.lower_bool_expr(value) {
                        self.push(IrInstruction::AssignBool {
                            local: stmt.name.clone(),
                            value,
                        });
                    } else {
                        self.lower_expr(value);
                    }
                }
                self.push(IrInstruction::Let {
                    local: stmt.name.clone(),
                });
                let inferred_ty = inferred_initializer_ty;
                let ty = match (declared_ty, inferred_ty) {
                    (Some(declared), _) => declared,
                    (None, Some(inferred)) => inferred,
                    (None, None) => IrValueTy::Unknown,
                };
                self.local_types.insert(stmt.name.clone(), ty);
                if let Some(scope) = self.scope_stack.last_mut() {
                    scope.push(stmt.name.clone());
                }
                if let Some(type_arg) = declared_channel_type_arg {
                    self.channel_local_type_args
                        .insert(stmt.name.clone(), type_arg);
                }
                if let Some(type_arg) = declared_box_type_arg {
                    self.box_local_type_args.insert(stmt.name.clone(), type_arg);
                }
                if let Some(type_arg) = declared_map_type_arg {
                    self.map_local_type_args.insert(stmt.name.clone(), type_arg);
                }
                if let Some(type_arg) = declared_vec_type_arg {
                    self.vec_local_type_args.insert(stmt.name.clone(), type_arg);
                }
                if let Some(Expr::Call(call)) = stmt.value.as_ref() {
                    if let Some(local) = consumed_cleanup_arg(call) {
                        self.consumed_locals.insert(local);
                    }
                }
                if matches!(
                    stmt.value.as_ref(),
                    Some(Expr::Unary(expr)) if matches!(expr.op, UnaryOp::Ref | UnaryOp::MutRef)
                ) {
                    self.live_borrows.push(stmt.name.clone());
                }
                self.locals.push(stmt.name.clone());
            }
            StmtData::Return(expr) => {
                if let Some(expr) = expr {
                    if matches!(expr, Expr::Block(block) if {
                        self.lower_return_block_value(block);
                        true
                    }) {
                    } else if !matches!(expr, Expr::If(if_expr) if self.lower_return_if_expr(if_expr))
                    {
                        self.lower_return_expr(expr);
                        self.set_terminator(IrTerminator::Return);
                    }
                } else {
                    self.append_return_drops(&HashSet::new());
                    self.push(IrInstruction::ReturnUnit);
                    self.set_terminator(IrTerminator::Return);
                }
            }
            StmtData::Break(expr) => {
                let referenced = expr.as_ref().map(referenced_locals).unwrap_or_default();
                if let Some(expr) = expr {
                    self.lower_expr(expr);
                }
                self.append_scope_exit_drops(&referenced);
                if let Some(targets) = self.loop_stack.last().copied() {
                    self.set_terminator(IrTerminator::Jump {
                        target: targets.break_block,
                    });
                    self.current = self.new_block();
                } else {
                    self.set_terminator(IrTerminator::Unreachable);
                }
            }
            StmtData::Continue => {
                self.append_scope_exit_drops(&HashSet::new());
                if let Some(targets) = self.loop_stack.last().copied() {
                    self.set_terminator(IrTerminator::Jump {
                        target: targets.continue_block,
                    });
                    self.current = self.new_block();
                } else {
                    self.set_terminator(IrTerminator::Unreachable);
                }
            }
            StmtData::If(control) => self.lower_if(control),
            StmtData::While(control) => self.lower_while(control),
            StmtData::Match(expr) => self.lower_match(expr),
            StmtData::For(stmt) => {
                self.lower_expr(&stmt.iterator);
                self.lower_loop(&stmt.body);
            }
            StmtData::Loop(block) => self.lower_loop(block),
            StmtData::Unsafe(block) => self.lower_block(block),
            StmtData::Expr(expr) => {
                if !self.lower_assignment(expr) {
                    self.lower_expr(expr);
                }
            }
            StmtData::Raw => self.push(IrInstruction::Expr {
                text: stmt.text.clone(),
            }),
        }
    }

    fn lower_if(&mut self, control: &ControlStmt) {
        let condition = control
            .condition
            .as_ref()
            .and_then(|expr| self.lower_bool_expr(expr));
        if condition.is_none() {
            if let Some(condition) = &control.condition {
                self.lower_expr(condition);
            }
        }

        let then_block = self.new_block();
        let else_block = self.new_block();
        let continue_block = self.new_block();
        self.set_terminator(IrTerminator::Branch {
            condition,
            then_block,
            else_block,
        });

        self.current = then_block;
        self.lower_block(&control.body);
        if !self.current_block_returns() {
            self.set_terminator(IrTerminator::Jump {
                target: continue_block,
            });
        }

        self.current = else_block;
        self.set_terminator(IrTerminator::Jump {
            target: continue_block,
        });

        self.current = continue_block;
    }

    fn lower_if_assignment(
        &mut self,
        local: &str,
        expr: &IfExpr,
        hint: Option<&IrValueTy>,
    ) -> bool {
        let Some(else_branch) = &expr.else_branch else {
            return false;
        };
        let condition = self.lower_bool_expr(&expr.condition);
        if condition.is_none() {
            self.lower_expr(&expr.condition);
        }

        let then_block = self.new_block();
        let else_block = self.new_block();
        let continue_block = self.new_block();
        self.set_terminator(IrTerminator::Branch {
            condition,
            then_block,
            else_block,
        });

        self.current = then_block;
        self.lower_assignment_block_value(local, &expr.then_branch, hint);
        if !self.current_block_returns() {
            self.set_terminator(IrTerminator::Jump {
                target: continue_block,
            });
        }

        self.current = else_block;
        self.lower_assignment_block_value(local, else_branch, hint);
        if !self.current_block_returns() {
            self.set_terminator(IrTerminator::Jump {
                target: continue_block,
            });
        }

        self.current = continue_block;
        true
    }

    fn lower_assignment_block_value(
        &mut self,
        local: &str,
        block: &Block,
        hint: Option<&IrValueTy>,
    ) {
        self.scope_stack.push(Vec::new());
        let final_expr_index = block
            .statements
            .last()
            .filter(|stmt| matches!(stmt.data, StmtData::Expr(_)))
            .map(|_| block.statements.len() - 1);
        for (index, stmt) in block.statements.iter().enumerate() {
            if Some(index) == final_expr_index {
                if let StmtData::Expr(expr) = &stmt.data {
                    self.assign_expr_to_local(local.to_string(), expr, hint);
                }
                break;
            }
            self.lower_stmt(stmt);
            if self.current_block_returns() {
                break;
            }
        }
        if !self.current_block_returns() {
            self.append_scope_exit_drops(&HashSet::new());
        }
        self.scope_stack.pop();
    }

    fn assign_expr_to_local(&mut self, local: String, expr: &Expr, hint: Option<&IrValueTy>) {
        if let Some(value) = self.lower_value_expr_with_hint(expr, hint) {
            match value {
                IrValueExpr::Int(value) => {
                    if let Some(consumed) = consumed_cleanup_arg_from_ir_int(&value) {
                        self.consumed_locals.insert(consumed);
                    }
                    self.push(IrInstruction::AssignInt { local, value });
                }
                IrValueExpr::Float(value) => self.push(IrInstruction::AssignFloat { local, value }),
                IrValueExpr::Bool(value) => self.push(IrInstruction::AssignBool { local, value }),
                IrValueExpr::String(value) => {
                    if let Some(consumed) = consumed_box_take_string_arg_from_ir_string(&value) {
                        self.consumed_locals.insert(consumed.clone());
                        self.push(IrInstruction::DropBoxStorage { local: consumed });
                    }
                    self.push(IrInstruction::AssignString { local, value });
                }
                IrValueExpr::Struct(value) => {
                    self.push(IrInstruction::AssignStruct { local, value })
                }
                IrValueExpr::Enum(value) => self.push(IrInstruction::AssignEnum { local, value }),
                IrValueExpr::Function(value) => {
                    self.push(IrInstruction::AssignFunction { local, value })
                }
            }
        } else {
            self.lower_expr(expr);
        }
    }

    fn lower_return_if_expr(&mut self, expr: &IfExpr) -> bool {
        let Some(else_branch) = &expr.else_branch else {
            return false;
        };
        let condition = self.lower_bool_expr(&expr.condition);
        if condition.is_none() {
            self.lower_expr(&expr.condition);
        }

        let then_block = self.new_block();
        let else_block = self.new_block();
        self.set_terminator(IrTerminator::Branch {
            condition,
            then_block,
            else_block,
        });

        self.current = then_block;
        self.lower_return_block_value(&expr.then_branch);

        self.current = else_block;
        self.lower_return_block_value(else_branch);
        true
    }

    fn lower_return_block_value(&mut self, block: &Block) {
        self.scope_stack.push(Vec::new());
        let final_expr_index = block
            .statements
            .last()
            .filter(|stmt| matches!(stmt.data, StmtData::Expr(_)))
            .map(|_| block.statements.len() - 1);
        for (index, stmt) in block.statements.iter().enumerate() {
            if Some(index) == final_expr_index {
                if let StmtData::Expr(expr) = &stmt.data {
                    self.lower_return_expr(expr);
                    self.set_terminator(IrTerminator::Return);
                }
                break;
            }
            self.lower_stmt(stmt);
            if self.current_block_returns() {
                break;
            }
        }
        if !self.current_block_returns() {
            self.append_return_drops(&HashSet::new());
            self.push(IrInstruction::ReturnUnit);
            self.set_terminator(IrTerminator::Return);
        }
        self.scope_stack.pop();
    }

    fn lower_return_expr(&mut self, expr: &Expr) {
        let referenced = referenced_locals(expr);
        let return_type = self.return_type.clone();
        if let Some((ty, value, arms)) = self.lower_enum_match_value(expr, Some(&return_type)) {
            match return_type {
                IrValueTy::Int(_) => {
                    let local = self.next_return_temp();
                    let match_value_drop =
                        enum_match_storage_drop(&value, &arms, &self.local_types);
                    self.push(IrInstruction::AssignEnumMatch {
                        local: local.clone(),
                        ty,
                        value,
                        arms,
                    });
                    self.push(IrInstruction::Let {
                        local: local.clone(),
                    });
                    self.local_types
                        .insert(local.clone(), IrValueTy::Int(IrIntTy::Int));
                    let emitted_drops = self.append_return_drops(&HashSet::new());
                    if let Some((local, ty)) = match_value_drop {
                        if !emitted_drops.iter().any(|dropped| dropped == &local) {
                            self.push(IrInstruction::Drop { local, ty });
                        }
                    }
                    self.push(IrInstruction::ReturnInt {
                        value: IrIntExpr::Local(local),
                    });
                }
                IrValueTy::Float64 => {
                    let local = self.next_return_temp();
                    let match_value_drop =
                        enum_match_storage_drop(&value, &arms, &self.local_types);
                    self.push(IrInstruction::AssignEnumMatch {
                        local: local.clone(),
                        ty,
                        value,
                        arms,
                    });
                    self.push(IrInstruction::Let {
                        local: local.clone(),
                    });
                    self.local_types.insert(local.clone(), IrValueTy::Float64);
                    let emitted_drops = self.append_return_drops(&HashSet::new());
                    if let Some((local, ty)) = match_value_drop {
                        if !emitted_drops.iter().any(|dropped| dropped == &local) {
                            self.push(IrInstruction::Drop { local, ty });
                        }
                    }
                    self.push(IrInstruction::ReturnFloat {
                        value: IrFloatExpr::Local(local),
                    });
                }
                IrValueTy::Bool => {
                    let local = self.next_return_temp();
                    let match_value_drop =
                        enum_match_storage_drop(&value, &arms, &self.local_types);
                    self.push(IrInstruction::AssignEnumMatch {
                        local: local.clone(),
                        ty,
                        value,
                        arms,
                    });
                    self.push(IrInstruction::Let {
                        local: local.clone(),
                    });
                    self.local_types.insert(local.clone(), IrValueTy::Bool);
                    let emitted_drops = self.append_return_drops(&HashSet::new());
                    if let Some((local, ty)) = match_value_drop {
                        if !emitted_drops.iter().any(|dropped| dropped == &local) {
                            self.push(IrInstruction::Drop { local, ty });
                        }
                    }
                    self.push(IrInstruction::ReturnBool {
                        value: IrBoolExpr::Local(local),
                    });
                }
                _ => {
                    self.append_return_drops(&referenced);
                    self.push(IrInstruction::ReturnEnumMatch {
                        ty,
                        value,
                        arms,
                        free_value_storage: true,
                    });
                }
            }
        } else if matches!(return_type, IrValueTy::Enum(_)) {
            if let Some(value) = self.lower_enum_expr_with_expected(expr, Some(&return_type)) {
                if self.return_expr_needs_temp_before_drops(&referenced) {
                    let local = self.next_return_temp();
                    self.push(IrInstruction::AssignEnum {
                        local: local.clone(),
                        value,
                    });
                    self.push(IrInstruction::Let {
                        local: local.clone(),
                    });
                    self.local_types
                        .insert(local.clone(), self.return_type.clone());
                    self.append_return_drops(&HashSet::new());
                    self.push(IrInstruction::ReturnEnum {
                        value: IrEnumExpr::Local(local),
                    });
                } else {
                    self.append_return_drops(&referenced);
                    self.push(IrInstruction::ReturnEnum { value });
                }
            } else {
                self.lower_expr(expr);
            }
        } else if is_string_ty(&self.return_type) {
            if let Some(value) = self.lower_string_expr(expr) {
                let returns_referenced_local = matches!(
                    &value,
                    IrStringExpr::Local(local) if referenced.contains(local)
                );
                if !returns_referenced_local
                    && self.return_expr_needs_temp_before_drops(&referenced)
                {
                    let local = self.next_return_temp();
                    self.push(IrInstruction::AssignString {
                        local: local.clone(),
                        value,
                    });
                    self.push(IrInstruction::Let {
                        local: local.clone(),
                    });
                    self.local_types
                        .insert(local.clone(), self.return_type.clone());
                    self.append_return_drops(&HashSet::new());
                    self.push(IrInstruction::ReturnString {
                        value: IrStringExpr::Local(local),
                    });
                } else {
                    self.append_return_drops(&referenced);
                    self.push(IrInstruction::ReturnString { value });
                }
            } else {
                self.lower_expr(expr);
            }
        } else if self.return_type == IrValueTy::Bool {
            if let Some(value) = self.lower_bool_expr(expr) {
                if self.scalar_return_expr_needs_temp_before_drops(&referenced) {
                    let local = self.next_return_temp();
                    self.push(IrInstruction::AssignBool {
                        local: local.clone(),
                        value,
                    });
                    self.push(IrInstruction::Let {
                        local: local.clone(),
                    });
                    self.local_types.insert(local.clone(), IrValueTy::Bool);
                    self.append_return_drops(&HashSet::new());
                    self.push(IrInstruction::ReturnBool {
                        value: IrBoolExpr::Local(local),
                    });
                } else {
                    self.append_return_drops(&referenced);
                    self.push(IrInstruction::ReturnBool { value });
                }
            } else {
                self.lower_expr(expr);
            }
        } else if self.return_type == IrValueTy::Float64 {
            if let Some(value) = self.lower_float_expr(expr) {
                if self.scalar_return_expr_needs_temp_before_drops(&referenced) {
                    let local = self.next_return_temp();
                    self.push(IrInstruction::AssignFloat {
                        local: local.clone(),
                        value,
                    });
                    self.push(IrInstruction::Let {
                        local: local.clone(),
                    });
                    self.local_types.insert(local.clone(), IrValueTy::Float64);
                    self.append_return_drops(&HashSet::new());
                    self.push(IrInstruction::ReturnFloat {
                        value: IrFloatExpr::Local(local),
                    });
                } else {
                    self.append_return_drops(&referenced);
                    self.push(IrInstruction::ReturnFloat { value });
                }
            } else {
                self.lower_expr(expr);
            }
        } else if matches!(self.return_type, IrValueTy::Function { .. }) {
            if let Some(value) = self
                .lower_function_assignment(expr)
                .or_else(|| self.lower_function_expr(expr))
            {
                if self.scalar_return_expr_needs_temp_before_drops(&referenced) {
                    let local = self.next_return_temp();
                    self.push(IrInstruction::AssignFunction {
                        local: local.clone(),
                        value,
                    });
                    self.push(IrInstruction::Let {
                        local: local.clone(),
                    });
                    self.local_types
                        .insert(local.clone(), self.return_type.clone());
                    self.append_return_drops(&HashSet::new());
                    self.push(IrInstruction::ReturnFunction {
                        value: IrFunctionExpr::Local(local),
                    });
                } else {
                    self.append_return_drops(&referenced);
                    self.push(IrInstruction::ReturnFunction { value });
                }
            } else {
                self.lower_expr(expr);
            }
        } else if matches!(
            self.return_type,
            IrValueTy::Boxed(_) | IrValueTy::Vec(_) | IrValueTy::Struct(_)
        ) {
            if let Some(value) = self.lower_struct_expr(expr) {
                if self.return_expr_needs_temp_before_drops(&referenced) {
                    let local = self.next_return_temp();
                    self.push(IrInstruction::AssignStruct {
                        local: local.clone(),
                        value,
                    });
                    self.push(IrInstruction::Let {
                        local: local.clone(),
                    });
                    self.local_types
                        .insert(local.clone(), self.return_type.clone());
                    self.append_return_drops(&HashSet::new());
                    self.push(IrInstruction::ReturnStruct {
                        value: IrStructExpr::Local(local),
                    });
                } else {
                    self.append_return_drops(&referenced);
                    self.push(IrInstruction::ReturnStruct { value });
                }
            } else {
                self.lower_expr(expr);
            }
        } else if let Some(value) = self.lower_int_expr(expr) {
            if self.scalar_return_expr_needs_temp_before_drops(&referenced) {
                let local = self.next_return_temp();
                self.push(IrInstruction::AssignInt {
                    local: local.clone(),
                    value,
                });
                self.push(IrInstruction::Let {
                    local: local.clone(),
                });
                self.local_types
                    .insert(local.clone(), IrValueTy::Int(IrIntTy::Int));
                self.append_return_drops(&HashSet::new());
                self.push(IrInstruction::ReturnInt {
                    value: IrIntExpr::Local(local),
                });
            } else {
                self.append_return_drops(&referenced);
                self.push(IrInstruction::ReturnInt { value });
            }
        } else if let Some(value) = self.lower_string_expr(expr) {
            self.append_return_drops(&referenced);
            self.push(IrInstruction::ReturnString { value });
        } else {
            self.lower_expr(expr);
        }
    }

    fn lower_while(&mut self, control: &ControlStmt) {
        let condition_block = self.new_block();
        let body_block = self.new_block();
        let exit_block = self.new_block();

        self.set_terminator(IrTerminator::Jump {
            target: condition_block,
        });

        self.current = condition_block;
        let condition = control
            .condition
            .as_ref()
            .and_then(|expr| self.lower_bool_expr(expr));
        if condition.is_none() {
            if let Some(condition) = &control.condition {
                self.lower_expr(condition);
            }
        }
        self.set_terminator(IrTerminator::Branch {
            condition,
            then_block: body_block,
            else_block: exit_block,
        });

        self.current = body_block;
        self.loop_stack.push(LoopTargets {
            continue_block: condition_block,
            break_block: exit_block,
        });
        self.lower_block(&control.body);
        self.loop_stack.pop();
        if !self.current_block_returns() {
            self.set_terminator(IrTerminator::Jump {
                target: condition_block,
            });
        }

        self.current = exit_block;
    }

    fn lower_loop(&mut self, block: &Block) {
        let body_block = self.new_block();
        let exit_block = self.new_block();
        self.set_terminator(IrTerminator::Jump { target: body_block });

        self.current = body_block;
        self.loop_stack.push(LoopTargets {
            continue_block: body_block,
            break_block: exit_block,
        });
        self.lower_block(block);
        self.loop_stack.pop();
        self.set_terminator(IrTerminator::Jump { target: body_block });

        self.current = exit_block;
    }

    fn lower_match(&mut self, expr: &MatchExpr) {
        let Some(value) = self.lower_enum_expr(&expr.value) else {
            self.lower_expr(&expr.value);
            return;
        };
        let arm_blocks: Vec<_> = expr.arms.iter().map(|_| self.new_block()).collect();
        let continue_block = self.new_block();
        let fallback = arm_blocks.last().copied().unwrap_or(continue_block);
        let switch_arms = expr
            .arms
            .iter()
            .zip(&arm_blocks)
            .filter_map(|(arm, target)| {
                let variant_name = pattern_variant_name(&arm.pattern)?;
                let variant = self.enum_variants.get(&variant_name)?;
                Some(IrSwitchArm {
                    tag: variant.tag,
                    target: *target,
                })
            })
            .collect::<Vec<_>>();
        self.set_terminator(IrTerminator::Switch {
            value: value.clone(),
            arms: switch_arms,
            fallback,
        });

        for (arm, block) in expr.arms.iter().zip(arm_blocks) {
            self.current = block;
            let variant = pattern_variant_name(&arm.pattern)
                .and_then(|variant_name| self.enum_variants.get(&variant_name).cloned());
            if let Some(variant) = variant {
                let bindings = pattern_bindings(&arm.pattern);
                for (payload_index, binding) in bindings.iter().enumerate() {
                    let payload_ty = variant
                        .payload_tys
                        .get(payload_index)
                        .cloned()
                        .unwrap_or(IrValueTy::Unknown);
                    self.push(IrInstruction::BindEnumPayload {
                        local: binding.clone(),
                        value: value.clone(),
                        payload_index,
                        payload_tys: variant.payload_tys.clone(),
                        payload_ty: payload_ty.clone(),
                    });
                    self.local_types.insert(binding.clone(), payload_ty);
                }
            }
            if !self.lower_assignment(&arm.body) {
                self.lower_expr(&arm.body);
            }
            if !self.current_block_returns() {
                self.set_terminator(IrTerminator::Jump {
                    target: continue_block,
                });
            }
        }
        self.current = continue_block;
    }

    fn lower_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Literal(Literal::Int(value)) => {
                if let Ok(value) = value.parse() {
                    self.push(IrInstruction::ConstInt { value });
                }
            }
            Expr::Missing | Expr::Ident(_) | Expr::Path(_) | Expr::Literal(_) | Expr::Raw(_) => {}
            Expr::Unary(expr) => {
                if matches!(expr.op, UnaryOp::Ref | UnaryOp::MutRef) {
                    if let Expr::Ident(name) = expr.expr.as_ref() {
                        self.push(IrInstruction::Borrow {
                            local: name.clone(),
                            mutable: expr.op == UnaryOp::MutRef,
                        });
                    }
                }
                self.lower_expr(&expr.expr);
            }
            Expr::Mut(expr) => {
                if let Expr::Ident(name) = expr.as_ref() {
                    self.push(IrInstruction::Borrow {
                        local: name.clone(),
                        mutable: true,
                    });
                }
                self.lower_expr(expr);
            }
            Expr::Binary(expr) => {
                self.lower_expr(&expr.left);
                self.lower_expr(&expr.right);
            }
            Expr::Index(expr) => {
                self.lower_expr(&expr.target);
                self.lower_expr(&expr.index);
            }
            Expr::Call(expr) => {
                if let Some(value) = print_string_literal(expr) {
                    self.push(IrInstruction::PrintString { value });
                    return;
                }
                if let Some(value) = self.print_string_expr(expr) {
                    self.push(IrInstruction::PrintStringExpr { value });
                    return;
                }
                if let Some(value) = self.print_int_expr(expr) {
                    self.push(IrInstruction::PrintInt { value });
                    return;
                }
                if let Some(condition) = self.assert_expr(expr) {
                    self.push(IrInstruction::Assert {
                        condition,
                        message: "assertion failed".to_string(),
                    });
                    return;
                }
                if let Some((fd, text)) = self.raw_write_expr(expr) {
                    self.push(IrInstruction::RawWrite { fd, text });
                    return;
                }
                if let Some((ptr, offset, value)) = self.raw_store8_expr(expr) {
                    self.push(IrInstruction::RawStore8 { ptr, offset, value });
                    return;
                }
                if let Some((ptr, offset, value)) = self.raw_store64_expr(expr) {
                    self.push(IrInstruction::RawStore64 { ptr, offset, value });
                    return;
                }
                if let Some((value, offset, byte)) = self.raw_string_store8_expr(expr) {
                    self.push(IrInstruction::RawStringStore8 {
                        value,
                        offset,
                        byte,
                    });
                    return;
                }
                if let Some(ptr) = self.raw_free_expr(expr) {
                    self.push(IrInstruction::RawFree { ptr });
                    return;
                }
                if let Expr::Ident(callee_name) = expr.callee.as_ref() {
                    if let Some(IrValueTy::Function { params, ret, .. }) =
                        self.local_types.get(callee_name).cloned()
                    {
                        if matches!(ret.as_ref(), IrValueTy::Unit) {
                            if let Some(args) = self.lower_value_args(&expr.args, &params) {
                                self.push(IrInstruction::IndirectCall {
                                    callee: IrFunctionExpr::Local(callee_name.clone()),
                                    args,
                                });
                                return;
                            }
                        }
                    }
                }
                if let Some((callee, args)) = self.lower_call_statement(expr) {
                    self.mark_ownership_transfer_args(&callee, &args);
                    self.push(IrInstruction::Call { callee, args });
                    return;
                }
                self.lower_expr(&expr.callee);
                for arg in &expr.args {
                    self.lower_expr(arg);
                }
            }
            Expr::Member(expr) => self.lower_expr(&expr.target),
            Expr::Await(expr) => self.lower_await(expr),
            Expr::Try(expr) => self.lower_try(expr),
            Expr::Struct(expr) => {
                for field in &expr.fields {
                    if let Some(value) = &field.value {
                        self.lower_expr(value);
                    }
                }
            }
            Expr::Object(expr) => {
                for field in &expr.fields {
                    self.lower_expr(&field.value);
                }
            }
            Expr::Closure(expr) => self.lower_closure(expr),
            Expr::Match(expr) => self.lower_match(expr),
            Expr::If(expr) => {
                self.lower_expr(&expr.condition);
                self.lower_block(&expr.then_branch);
                if let Some(else_branch) = &expr.else_branch {
                    self.lower_block(else_branch);
                }
            }
            Expr::Block(block) => self.lower_block(block),
        }
    }

    fn lower_call_statement(&mut self, expr: &CallExpr) -> Option<(String, Vec<IrValueExpr>)> {
        let method_receiver = if let Expr::Member(member) = expr.callee.as_ref() {
            Some(member.target.as_ref())
        } else {
            None
        };
        let callee = match expr.callee.as_ref() {
            Expr::Member(member) => member.member.as_str(),
            _ => direct_callee_name(&expr.callee)?,
        };
        let callee = self
            .specialize_call_callee(callee, expr)
            .unwrap_or_else(|| callee.to_string());
        let callee = if method_receiver.is_some() && !self.function_sigs.contains_key(&callee) {
            let suffix = format!("__{callee}");
            self.function_sigs
                .keys()
                .find(|name| name.ends_with(&suffix))
                .cloned()
                .unwrap_or(callee)
        } else {
            callee
        };
        let sig = self.function_sigs.get(&callee)?.clone();
        let args = if let Some(receiver) = method_receiver {
            self.lower_method_value_args(receiver, &expr.args, &sig.params)?
        } else {
            self.lower_value_args(&expr.args, &sig.params)?
        };
        Some((callee, args))
    }

    fn mark_ownership_transfer_args(&mut self, callee: &str, args: &[IrValueExpr]) {
        for index in ownership_transfer_arg_indexes(callee) {
            if let Some(IrValueExpr::String(IrStringExpr::Local(local))) = args.get(*index) {
                self.consumed_locals.insert(local.clone());
            }
        }
    }

    fn lower_string_expr(&mut self, expr: &Expr) -> Option<IrStringExpr> {
        self.lower_string_expr_with_channel_type_arg(expr, None)
    }

    fn lower_string_expr_with_channel_type_arg(
        &mut self,
        expr: &Expr,
        expected_channel_type_arg: Option<&str>,
    ) -> Option<IrStringExpr> {
        match expr {
            Expr::Literal(Literal::String(value)) => Some(IrStringExpr::Literal(value.clone())),
            Expr::Await(expr) => {
                self.record_suspend_point();
                self.lower_string_expr(expr)
            }
            Expr::Ident(name) if self.local_types.get(name).is_some_and(is_string_ty) => {
                Some(IrStringExpr::Local(name.clone()))
            }
            Expr::Member(expr) => {
                let field_ty = self.member_ty(expr)?;
                if is_string_ty(&field_ty) {
                    member_field_names(expr)
                        .map(|(base, field)| IrStringExpr::Field { base, field })
                } else {
                    None
                }
            }
            Expr::Binary(expr) if expr.op == BinaryOp::Add => Some(IrStringExpr::Concat {
                left: Box::new(self.lower_string_expr(&expr.left)?),
                right: Box::new(self.lower_string_expr(&expr.right)?),
            }),
            Expr::Call(expr) => {
                if matches!(expr.callee.as_ref(), Expr::Ident(name) if name == "raw_alloc_string") {
                    return match expr.args.as_slice() {
                        [size] => Some(IrStringExpr::RawAlloc {
                            size: Box::new(self.lower_int_expr(size)?),
                        }),
                        _ => None,
                    };
                }
                if matches!(expr.callee.as_ref(), Expr::Ident(name) if name == "raw_string_concat")
                {
                    return match expr.args.as_slice() {
                        [left, right] => Some(IrStringExpr::Concat {
                            left: Box::new(self.lower_string_expr(left)?),
                            right: Box::new(self.lower_string_expr(right)?),
                        }),
                        _ => None,
                    };
                }
                if matches!(expr.callee.as_ref(), Expr::Ident(name) if name == "raw_int_to_string")
                {
                    return match expr.args.as_slice() {
                        [value] => Some(IrStringExpr::IntToString(Box::new(
                            self.lower_int_expr(value)?,
                        ))),
                        _ => None,
                    };
                }
                if matches!(expr.callee.as_ref(), Expr::Ident(name) if name == "raw_string_from_ptr")
                {
                    return match expr.args.as_slice() {
                        [value] => {
                            Some(IrStringExpr::FromPtr(Box::new(self.lower_int_expr(value)?)))
                        }
                        _ => None,
                    };
                }
                let function_value_ty = match expr.callee.as_ref() {
                    Expr::Ident(callee_name)
                        if matches!(
                            self.local_types.get(callee_name),
                            Some(IrValueTy::Function { .. })
                        ) =>
                    {
                        self.local_types.get(callee_name).cloned()
                    }
                    Expr::Member(_) => self.infer_expr_ty(Some(&expr.callee)),
                    _ => None,
                };
                if let Some(IrValueTy::Function { params, ret, .. }) = function_value_ty {
                    if is_string_ty(ret.as_ref()) {
                        let args = self.lower_value_args(&expr.args, &params)?;
                        return Some(IrStringExpr::IndirectCall {
                            callee: self.lower_function_expr(&expr.callee)?,
                            args,
                        });
                    }
                }
                let callee = call_callee_name(&expr.callee)?;
                let callee = self
                    .specialize_call_callee_with_channel_type_arg(
                        callee,
                        expr,
                        expected_channel_type_arg,
                    )
                    .unwrap_or_else(|| callee.to_string());
                let sig = self.function_sigs.get(&callee)?;
                if !is_string_ty(&sig.ret) {
                    return None;
                }
                Some(IrStringExpr::Call {
                    callee,
                    args: self.lower_value_args(&expr.args, &sig.params)?,
                })
            }
            _ => None,
        }
    }

    fn lower_struct_expr(&mut self, expr: &Expr) -> Option<IrStructExpr> {
        self.lower_struct_expr_with_channel_type_arg(expr, None, None)
    }

    fn lower_struct_expr_with_channel_type_arg(
        &mut self,
        expr: &Expr,
        expected_channel_type_arg: Option<&str>,
        expected_ty: Option<&IrValueTy>,
    ) -> Option<IrStructExpr> {
        match expr {
            Expr::Await(expr) => {
                self.record_suspend_point();
                self.lower_struct_expr_with_channel_type_arg(
                    expr,
                    expected_channel_type_arg,
                    expected_ty,
                )
            }
            Expr::Ident(name) => match self.local_types.get(name) {
                Some(IrValueTy::Boxed(_) | IrValueTy::Vec(_) | IrValueTy::Struct(_)) => {
                    Some(IrStructExpr::Local(name.clone()))
                }
                _ => None,
            },
            Expr::Member(expr) => {
                let field_ty = self.member_ty(expr)?;
                if matches!(
                    field_ty,
                    IrValueTy::Boxed(_) | IrValueTy::Vec(_) | IrValueTy::Struct(_)
                ) {
                    member_field_names(expr)
                        .map(|(base, field)| IrStructExpr::Field { base, field })
                } else {
                    None
                }
            }
            Expr::Struct(expr) => {
                let struct_name = match expected_ty {
                    Some(IrValueTy::Struct(name)) => name.clone(),
                    _ => expr.name.clone(),
                };
                Some(IrStructExpr::Construct {
                    name: struct_name.clone(),
                    fields: expr
                        .fields
                        .iter()
                        .filter_map(|field| {
                            let field_ty = self
                                .struct_fields
                                .get(&struct_name)
                                .and_then(|fields| fields.get(&field.name));
                            Some(IrStructFieldValue {
                                name: field.name.clone(),
                                value: self
                                    .lower_value_expr_with_hint(field.value.as_ref()?, field_ty)?,
                            })
                        })
                        .collect(),
                })
            }
            Expr::Call(expr) => {
                let function_value_ty = match expr.callee.as_ref() {
                    Expr::Ident(callee_name)
                        if matches!(
                            self.local_types.get(callee_name),
                            Some(IrValueTy::Function { .. })
                        ) =>
                    {
                        self.local_types.get(callee_name).cloned()
                    }
                    Expr::Member(_) => self.infer_expr_ty(Some(&expr.callee)),
                    _ => None,
                };
                if let Some(IrValueTy::Function { params, ret, .. }) = function_value_ty {
                    if matches!(
                        ret.as_ref(),
                        IrValueTy::Boxed(_) | IrValueTy::Vec(_) | IrValueTy::Struct(_)
                    ) {
                        let args = self.lower_value_args(&expr.args, &params)?;
                        return Some(IrStructExpr::IndirectCall {
                            callee: self.lower_function_expr(&expr.callee)?,
                            args,
                        });
                    }
                }
                let method_receiver = if let Expr::Member(member) = expr.callee.as_ref() {
                    Some(member.target.as_ref())
                } else {
                    None
                };
                let callee = match expr.callee.as_ref() {
                    Expr::Member(member) => member.member.as_str(),
                    _ => direct_callee_name(&expr.callee)?,
                };
                let callee = self
                    .specialize_call_callee_with_channel_type_arg(
                        callee,
                        expr,
                        expected_channel_type_arg,
                    )
                    .unwrap_or_else(|| callee.to_string());
                let callee =
                    if method_receiver.is_some() && !self.function_sigs.contains_key(&callee) {
                        let suffix = format!("__{callee}");
                        self.function_sigs
                            .keys()
                            .find(|name| name.ends_with(&suffix))
                            .cloned()
                            .unwrap_or(callee)
                    } else {
                        callee
                    };
                let sig = self.function_sigs.get(&callee)?;
                if is_thread_spawn_callee(&callee) {
                    if let IrValueTy::Struct(name) = &sig.ret {
                        if name.ends_with("JoinHandle") || name == "JoinHandle" {
                            let [task] = expr.args.as_slice() else {
                                return None;
                            };
                            let (task, captures) = self.lower_thread_spawn_task(task)?;
                            return Some(IrStructExpr::Construct {
                                name: name.clone(),
                                fields: vec![IrStructFieldValue {
                                    name: "raw".to_string(),
                                    value: IrValueExpr::Int(IrIntExpr::RawThreadSpawn {
                                        task,
                                        captures,
                                    }),
                                }],
                            });
                        }
                    }
                }
                if !matches!(
                    sig.ret,
                    IrValueTy::Boxed(_) | IrValueTy::Vec(_) | IrValueTy::Struct(_)
                ) {
                    return None;
                }
                let args = if let Some(receiver) = method_receiver {
                    self.lower_method_value_args(receiver, &expr.args, &sig.params)?
                } else {
                    self.lower_value_args(&expr.args, &sig.params)?
                };
                Some(IrStructExpr::Call { callee, args })
            }
            _ => None,
        }
    }

    fn lower_enum_expr(&mut self, expr: &Expr) -> Option<IrEnumExpr> {
        self.lower_enum_expr_with_expected(expr, None)
    }

    fn lower_enum_expr_with_expected(
        &mut self,
        expr: &Expr,
        expected: Option<&IrValueTy>,
    ) -> Option<IrEnumExpr> {
        match expr {
            Expr::Ident(name) if self.enum_variants.contains_key(name) => {
                let variant = self.expected_enum_variant(expected, name)?;
                if !variant.payload_tys.is_empty() {
                    return None;
                }
                Some(IrEnumExpr::Construct {
                    enum_name: variant.enum_name.clone(),
                    variant: name.clone(),
                    tag: variant.tag,
                    payloads: Vec::new(),
                })
            }
            Expr::Ident(name) => match self.local_types.get(name) {
                Some(IrValueTy::Enum(_)) => Some(IrEnumExpr::Local(name.clone())),
                _ => None,
            },
            Expr::Call(expr) => {
                let variant_name = call_callee_name(&expr.callee)?;
                if let Some(variant) = self.expected_enum_variant(expected, variant_name) {
                    if expr.args.len() != variant.payload_tys.len() {
                        return None;
                    }
                    let payloads = expr
                        .args
                        .iter()
                        .zip(&variant.payload_tys)
                        .map(|(arg, ty)| self.lower_value_expr_with_hint(arg, Some(ty)))
                        .collect::<Option<Vec<_>>>()?;
                    return Some(IrEnumExpr::Construct {
                        enum_name: variant.enum_name.clone(),
                        variant: variant_name.to_string(),
                        tag: variant.tag,
                        payloads,
                    });
                }
                let function_value_ty = match expr.callee.as_ref() {
                    Expr::Ident(callee_name)
                        if matches!(
                            self.local_types.get(callee_name),
                            Some(IrValueTy::Function { .. })
                        ) =>
                    {
                        self.local_types.get(callee_name).cloned()
                    }
                    Expr::Member(_) => self.infer_expr_ty(Some(&expr.callee)),
                    _ => None,
                };
                if let Some(IrValueTy::Function { params, ret, .. }) = function_value_ty {
                    if matches!(ret.as_ref(), IrValueTy::Enum(_)) {
                        let args = self.lower_value_args(&expr.args, &params)?;
                        return Some(IrEnumExpr::IndirectCall {
                            callee: self.lower_function_expr(&expr.callee)?,
                            args,
                        });
                    }
                }
                let callee = self
                    .specialize_call_callee(variant_name, expr)
                    .unwrap_or_else(|| variant_name.to_string());
                let sig = self.function_sigs.get(&callee)?.clone();
                if !matches!(sig.ret, IrValueTy::Enum(_)) {
                    return None;
                }
                let args = self.lower_value_args(&expr.args, &sig.params)?;
                Some(IrEnumExpr::Call { callee, args })
            }
            _ => None,
        }
    }

    fn expected_enum_variant(
        &self,
        expected: Option<&IrValueTy>,
        variant_name: &str,
    ) -> Option<EnumVariantSig> {
        if let Some(IrValueTy::Enum(enum_name)) = expected {
            if let Some(variant) = self
                .enum_variants_by_enum
                .get(enum_name)
                .and_then(|variants| variants.get(variant_name))
            {
                return Some(variant.clone());
            }
        }
        self.enum_variants.get(variant_name).cloned()
    }

    fn lower_enum_match_value(
        &mut self,
        expr: &Expr,
        hint: Option<&IrValueTy>,
    ) -> Option<(IrValueTy, IrEnumExpr, Vec<IrEnumMatchArm>)> {
        let Expr::Match(expr) = expr else {
            return None;
        };
        let value = self.lower_enum_expr(&expr.value)?;
        let result_ty = hint.cloned().or_else(|| {
            expr.arms
                .iter()
                .find_map(|arm| self.infer_expr_ty(Some(&arm.body)))
        })?;
        let mut arms = Vec::new();
        for arm in &expr.arms {
            let variant_name = pattern_variant_name(&arm.pattern)?;
            let variant = self.enum_variants.get(&variant_name)?.clone();
            let bindings = pattern_bindings(&arm.pattern);
            let previous_bindings = bindings
                .iter()
                .enumerate()
                .map(|(index, binding)| {
                    let previous = self.local_types.get(binding).cloned();
                    if let Some(payload_ty) = variant.payload_tys.get(index) {
                        self.local_types.insert(binding.clone(), payload_ty.clone());
                    }
                    (binding.clone(), previous)
                })
                .collect::<Vec<_>>();
            let body = self.lower_value_expr_with_match_bindings(
                &arm.body,
                &result_ty,
                &bindings,
                &variant.payload_tys,
            );
            for (binding, previous) in previous_bindings {
                if let Some(previous) = previous {
                    self.local_types.insert(binding, previous);
                } else {
                    self.local_types.remove(&binding);
                }
            }
            arms.push(IrEnumMatchArm {
                variant: variant_name,
                tag: variant.tag,
                payload_tys: variant.payload_tys.clone(),
                body: body?,
                bindings,
            });
        }
        Some((result_ty, value, arms))
    }

    fn lower_value_expr_with_match_bindings(
        &mut self,
        expr: &Expr,
        hint: &IrValueTy,
        bindings: &[String],
        payload_tys: &[IrValueTy],
    ) -> Option<IrValueExpr> {
        if let Expr::Ident(name) = expr {
            if let Some((index, _)) = bindings
                .iter()
                .enumerate()
                .find(|(_, binding)| *binding == name)
            {
                let binding_ty = payload_tys.get(index).unwrap_or(hint);
                let ty = if matches!(binding_ty, IrValueTy::Unknown) {
                    hint
                } else {
                    binding_ty
                };
                return match ty {
                    IrValueTy::String | IrValueTy::BorrowedString | IrValueTy::OwnedString => {
                        Some(IrValueExpr::String(IrStringExpr::Local(name.clone())))
                    }
                    IrValueTy::Boxed(_) | IrValueTy::Vec(_) | IrValueTy::Struct(_) => {
                        Some(IrValueExpr::Struct(IrStructExpr::Local(name.clone())))
                    }
                    IrValueTy::Enum(_) => Some(IrValueExpr::Enum(IrEnumExpr::Local(name.clone()))),
                    IrValueTy::Function { .. } => {
                        Some(IrValueExpr::Function(IrFunctionExpr::Local(name.clone())))
                    }
                    IrValueTy::Bool => Some(IrValueExpr::Bool(IrBoolExpr::Local(name.clone()))),
                    IrValueTy::Int(_) => Some(IrValueExpr::Int(IrIntExpr::Local(name.clone()))),
                    IrValueTy::Float64 => {
                        Some(IrValueExpr::Float(IrFloatExpr::Local(name.clone())))
                    }
                    IrValueTy::Unknown | IrValueTy::Unit => None,
                };
            }
        }
        self.lower_value_expr_with_hint(expr, Some(hint))
    }

    fn lower_value_args(
        &mut self,
        args: &[Expr],
        expected: &[IrValueTy],
    ) -> Option<Vec<IrValueExpr>> {
        args.iter()
            .enumerate()
            .map(|(index, arg)| self.lower_value_expr_with_hint(arg, expected.get(index)))
            .collect()
    }

    fn lower_method_value_args(
        &mut self,
        receiver: &Expr,
        args: &[Expr],
        expected: &[IrValueTy],
    ) -> Option<Vec<IrValueExpr>> {
        let mut values = Vec::new();
        values.push(self.lower_value_expr_with_hint(receiver, expected.first())?);
        for (index, arg) in args.iter().enumerate() {
            values.push(self.lower_value_expr_with_hint(arg, expected.get(index + 1))?);
        }
        Some(values)
    }

    fn lower_value_expr_with_hint(
        &mut self,
        expr: &Expr,
        hint: Option<&IrValueTy>,
    ) -> Option<IrValueExpr> {
        let hint = hint.filter(|ty| !is_generic_placeholder_ty(ty));
        if let Expr::Ident(name) = expr {
            if let Some(actual) = self.local_types.get(name) {
                if hint.is_none_or(|ty| {
                    matches!(ty, IrValueTy::Unknown) || is_generic_placeholder_ty(ty)
                }) {
                    return Some(local_value_expr(name, Some(actual)));
                }
            }
        }
        match hint {
            Some(ty) if is_string_ty(ty) => self.lower_string_expr(expr).map(IrValueExpr::String),
            Some(IrValueTy::Boxed(_) | IrValueTy::Vec(_) | IrValueTy::Struct(_)) => {
                self.lower_struct_expr(expr).map(IrValueExpr::Struct)
            }
            Some(ty @ IrValueTy::Enum(_)) => self
                .lower_enum_expr_with_expected(expr, Some(ty))
                .map(IrValueExpr::Enum),
            Some(IrValueTy::Function { .. }) => self
                .lower_function_assignment(expr)
                .or_else(|| self.lower_function_expr(expr))
                .map(IrValueExpr::Function),
            Some(IrValueTy::Bool) => self.lower_bool_expr(expr).map(IrValueExpr::Bool),
            Some(IrValueTy::Int(_)) => self.lower_int_expr(expr).map(IrValueExpr::Int),
            Some(IrValueTy::Float64) => self.lower_float_expr(expr).map(IrValueExpr::Float),
            _ => self
                .lower_struct_expr(expr)
                .map(IrValueExpr::Struct)
                .or_else(|| self.lower_enum_expr(expr).map(IrValueExpr::Enum))
                .or_else(|| self.lower_function_expr(expr).map(IrValueExpr::Function))
                .or_else(|| self.lower_string_expr(expr).map(IrValueExpr::String))
                .or_else(|| self.lower_bool_expr(expr).map(IrValueExpr::Bool))
                .or_else(|| self.lower_int_expr(expr).map(IrValueExpr::Int))
                .or_else(|| self.lower_float_expr(expr).map(IrValueExpr::Float)),
        }
    }

    fn lower_function_expr(&mut self, expr: &Expr) -> Option<IrFunctionExpr> {
        match expr {
            Expr::Ident(name) => {
                if matches!(self.local_types.get(name), Some(IrValueTy::Function { .. })) {
                    return Some(IrFunctionExpr::Local(name.clone()));
                }
                if self.function_sigs.contains_key(name) {
                    return Some(IrFunctionExpr::Named(name.clone()));
                }
            }
            Expr::Member(expr) => {
                let field_ty = self.member_ty(expr)?;
                if matches!(field_ty, IrValueTy::Function { .. }) {
                    return member_field_names(expr)
                        .map(|(base, field)| IrFunctionExpr::Field { base, field });
                }
            }
            Expr::Call(call) if matches!(call.callee.as_ref(), Expr::Ident(name) if name == "raw_function_from_ptr" || name == "raw_function_from_ptr_int" || name == "raw_function_from_ptr_handler" || name == "raw_function_from_ptr_request_handler" || name == "raw_function_from_ptr_response_handler") => {
                if let [ptr] = call.args.as_slice() {
                    return Some(IrFunctionExpr::FromPtr(Box::new(self.lower_int_expr(ptr)?)));
                }
            }
            Expr::Call(call) => {
                let callee = call_callee_name(&call.callee)?;
                let callee = self
                    .specialize_call_callee(callee, call)
                    .unwrap_or_else(|| callee.to_string());
                let sig = self.function_sigs.get(&callee)?;
                if !matches!(sig.ret, IrValueTy::Function { .. }) {
                    return None;
                }
                return Some(IrFunctionExpr::Call {
                    callee,
                    args: self.lower_value_args(&call.args, &sig.params)?,
                });
            }
            _ => {}
        }
        None
    }

    fn lower_function_assignment(&mut self, expr: &Expr) -> Option<IrFunctionExpr> {
        match expr {
            Expr::Ident(_) => self.lower_function_expr(expr),
            Expr::Closure(closure) => self.lower_non_capturing_closure_function(closure),
            _ => None,
        }
    }

    fn lower_thread_spawn_task(
        &mut self,
        expr: &Expr,
    ) -> Option<(IrFunctionExpr, Vec<IrValueExpr>)> {
        match expr {
            Expr::Closure(closure) => self.lower_thread_closure_function(closure),
            _ => self
                .lower_function_assignment(expr)
                .or_else(|| self.lower_function_expr(expr))
                .map(|task| (task, Vec::new())),
        }
    }

    fn lower_thread_closure_function(
        &mut self,
        expr: &ClosureExpr,
    ) -> Option<(IrFunctionExpr, Vec<IrValueExpr>)> {
        let captures = closure_captures(expr, &self.locals);
        if captures.is_empty() {
            return self
                .lower_non_capturing_closure_function(expr)
                .map(|task| (task, Vec::new()));
        }
        if expr.is_async || !expr.params.is_empty() {
            return None;
        }
        let capture_values = captures
            .iter()
            .map(|capture| {
                let ty = self.local_types.get(capture)?;
                match ty {
                    IrValueTy::String | IrValueTy::BorrowedString | IrValueTy::OwnedString => {
                        Some(IrValueExpr::String(IrStringExpr::Local(capture.clone())))
                    }
                    IrValueTy::Struct(_) => {
                        Some(IrValueExpr::Struct(IrStructExpr::Local(capture.clone())))
                    }
                    IrValueTy::Enum(_) => {
                        Some(IrValueExpr::Enum(IrEnumExpr::Local(capture.clone())))
                    }
                    IrValueTy::Bool => Some(IrValueExpr::Bool(IrBoolExpr::Local(capture.clone()))),
                    IrValueTy::Int(_) => Some(IrValueExpr::Int(IrIntExpr::Local(capture.clone()))),
                    IrValueTy::Float64 => {
                        Some(IrValueExpr::Float(IrFloatExpr::Local(capture.clone())))
                    }
                    _ => None,
                }
            })
            .collect::<Option<Vec<_>>>()?;
        let index = self.closure_index;
        self.closure_index += 1;
        let name = format!("__closure_{}_{}", self.function_name, index);
        let mut local_types = HashMap::new();
        local_types.insert("__env".to_string(), IrValueTy::Int(IrIntTy::Int));
        let mut instructions = Vec::new();
        for (index, capture) in captures.iter().enumerate() {
            let ty = self.local_types.get(capture)?.clone();
            let offset = ((index + 1) * 8) as i32;
            local_types.insert(capture.clone(), ty.clone());
            match ty {
                IrValueTy::String | IrValueTy::BorrowedString | IrValueTy::OwnedString => {
                    instructions.push(IrInstruction::AssignString {
                        local: capture.clone(),
                        value: IrStringExpr::EnvLoad { offset },
                    })
                }
                IrValueTy::Struct(_) => instructions.push(IrInstruction::AssignStruct {
                    local: capture.clone(),
                    value: IrStructExpr::EnvLoad { offset },
                }),
                IrValueTy::Enum(_) => instructions.push(IrInstruction::AssignEnum {
                    local: capture.clone(),
                    value: IrEnumExpr::EnvLoad { offset },
                }),
                IrValueTy::Bool => instructions.push(IrInstruction::AssignBool {
                    local: capture.clone(),
                    value: IrBoolExpr::EnvLoad { offset },
                }),
                IrValueTy::Int(_) => instructions.push(IrInstruction::AssignInt {
                    local: capture.clone(),
                    value: IrIntExpr::EnvLoad { offset },
                }),
                IrValueTy::Float64 => instructions.push(IrInstruction::AssignFloat {
                    local: capture.clone(),
                    value: IrFloatExpr::EnvLoad { offset },
                }),
                _ => return None,
            }
        }
        let captured_drop_locals = captures
            .iter()
            .filter_map(|capture| {
                let ty = self.local_types.get(capture)?;
                matches!(ty, IrValueTy::Struct(_) | IrValueTy::Enum(_)).then(|| capture.clone())
            })
            .collect::<Vec<_>>();
        let mut lowerer = FunctionLowerer {
            blocks: vec![IrBlock {
                id: 0,
                instructions,
                terminator: IrTerminator::Return,
            }],
            current: 0,
            loop_stack: Vec::new(),
            locals: std::iter::once("__env".to_string())
                .chain(captures.iter().cloned())
                .collect(),
            local_types,
            channel_local_type_args: captures
                .iter()
                .filter_map(|capture| {
                    self.channel_local_type_args
                        .get(capture)
                        .map(|type_arg| (capture.clone(), type_arg.clone()))
                })
                .collect(),
            box_local_type_args: captures
                .iter()
                .filter_map(|capture| {
                    self.box_local_type_args
                        .get(capture)
                        .map(|type_arg| (capture.clone(), type_arg.clone()))
                })
                .collect(),
            map_local_type_args: captures
                .iter()
                .filter_map(|capture| {
                    self.map_local_type_args
                        .get(capture)
                        .map(|type_arg| (capture.clone(), type_arg.clone()))
                })
                .collect(),
            vec_local_type_args: captures
                .iter()
                .filter_map(|capture| {
                    self.vec_local_type_args
                        .get(capture)
                        .map(|type_arg| (capture.clone(), type_arg.clone()))
                })
                .collect(),
            function_sigs: self.function_sigs,
            struct_fields: self.struct_fields,
            enum_variants: self.enum_variants,
            enum_variants_by_enum: self.enum_variants_by_enum,
            enum_names: self.enum_names,
            generic_struct_names: self.generic_struct_names,
            return_type: IrValueTy::Unit,
            live_borrows: Vec::new(),
            suspend_points: 0,
            module: self.module,
            function_name: name.clone(),
            closure_index: 0,
            generated_functions: Vec::new(),
            return_temp_index: 0,
            assignment_temp_index: 0,
            consumed_locals: HashSet::new(),
            return_drop_locals: captured_drop_locals.clone(),
            scope_stack: Vec::new(),
        };
        lowerer.lower_block(&expr.body);
        lowerer.append_drops(&captured_drop_locals);
        let nested = lowerer.generated_functions;
        self.generated_functions.extend(nested);
        self.generated_functions.push(IrFunction {
            module: self.module,
            name: name.clone(),
            is_async: false,
            future_state: None,
            params: vec!["__env".to_string()],
            param_types: vec![IrValueTy::Int(IrIntTy::Int)],
            return_type: IrValueTy::Unit,
            blocks: lowerer.blocks,
        });
        Some((IrFunctionExpr::Named(name), capture_values))
    }

    fn print_string_expr(&mut self, expr: &CallExpr) -> Option<IrStringExpr> {
        if !matches!(expr.callee.as_ref(), Expr::Ident(name) if name == "print") {
            return None;
        }
        match expr.args.as_slice() {
            [value] => self.lower_string_expr(value),
            _ => None,
        }
    }

    fn print_int_expr(&mut self, expr: &CallExpr) -> Option<IrIntExpr> {
        if !matches!(expr.callee.as_ref(), Expr::Ident(name) if name == "print") {
            return None;
        }
        match expr.args.as_slice() {
            [value] => self.lower_int_expr(value),
            _ => None,
        }
    }

    fn assert_expr(&mut self, expr: &CallExpr) -> Option<IrBoolExpr> {
        if !matches!(expr.callee.as_ref(), Expr::Ident(name) if name == "assert") {
            return None;
        }
        match expr.args.as_slice() {
            [value] => self.lower_bool_expr(value),
            _ => None,
        }
    }

    fn raw_write_expr(&mut self, expr: &CallExpr) -> Option<(IrIntExpr, IrStringExpr)> {
        if !matches!(expr.callee.as_ref(), Expr::Ident(name) if name == "raw_write") {
            return None;
        }
        match expr.args.as_slice() {
            [fd, text] => Some((self.lower_int_expr(fd)?, self.lower_string_expr(text)?)),
            _ => None,
        }
    }

    fn raw_store8_expr(&mut self, expr: &CallExpr) -> Option<(IrIntExpr, IrIntExpr, IrIntExpr)> {
        if !matches!(expr.callee.as_ref(), Expr::Ident(name) if name == "raw_store8") {
            return None;
        }
        match expr.args.as_slice() {
            [ptr, offset, value] => Some((
                self.lower_int_expr(ptr)?,
                self.lower_int_expr(offset)?,
                self.lower_int_expr(value)?,
            )),
            _ => None,
        }
    }

    fn raw_store64_expr(&mut self, expr: &CallExpr) -> Option<(IrIntExpr, IrIntExpr, IrIntExpr)> {
        if !matches!(expr.callee.as_ref(), Expr::Ident(name) if name == "raw_store64") {
            return None;
        }
        match expr.args.as_slice() {
            [ptr, offset, value] => Some((
                self.lower_int_expr(ptr)?,
                self.lower_int_expr(offset)?,
                self.lower_int_expr(value)?,
            )),
            _ => None,
        }
    }

    fn raw_string_store8_expr(
        &mut self,
        expr: &CallExpr,
    ) -> Option<(IrStringExpr, IrIntExpr, IrIntExpr)> {
        if !matches!(expr.callee.as_ref(), Expr::Ident(name) if name == "raw_string_store8") {
            return None;
        }
        match expr.args.as_slice() {
            [value, offset, byte] => Some((
                self.lower_string_expr(value)?,
                self.lower_int_expr(offset)?,
                self.lower_int_expr(byte)?,
            )),
            _ => None,
        }
    }

    fn raw_free_expr(&mut self, expr: &CallExpr) -> Option<IrIntExpr> {
        if !matches!(expr.callee.as_ref(), Expr::Ident(name) if name == "raw_free") {
            return None;
        }
        match expr.args.as_slice() {
            [ptr] => self.lower_int_expr(ptr),
            _ => None,
        }
    }

    fn infer_expr_ty(&self, expr: Option<&Expr>) -> Option<IrValueTy> {
        let expr = expr?;
        match expr {
            Expr::Literal(Literal::Int(_)) => Some(IrValueTy::Int(IrIntTy::Int)),
            Expr::Literal(Literal::Float(_)) => Some(IrValueTy::Float64),
            Expr::Literal(Literal::Bool(_)) => Some(IrValueTy::Bool),
            Expr::Literal(Literal::String(_)) => Some(IrValueTy::String),
            Expr::Ident(name) => self
                .local_types
                .get(name)
                .cloned()
                .or_else(|| self.function_sigs.get(name).map(ir_fn_ty_from_sig)),
            Expr::Struct(expr) => Some(self.infer_struct_literal_ty(expr)),
            Expr::Closure(expr) => Some(IrValueTy::Function {
                params: expr
                    .params
                    .iter()
                    .map(|param| {
                        param
                            .ty_expr
                            .as_ref()
                            .map(|ty| {
                                ir_param_ty_from_type(
                                    ty,
                                    self.enum_names,
                                    self.generic_struct_names,
                                )
                            })
                            .unwrap_or(IrValueTy::Unknown)
                    })
                    .collect(),
                ret: Box::new(
                    expr.return_type_expr
                        .as_ref()
                        .map(|ty| {
                            ir_return_ty_from_type(ty, self.enum_names, self.generic_struct_names)
                        })
                        .unwrap_or(IrValueTy::Unit),
                ),
                is_async: expr.is_async,
            }),
            Expr::Unary(expr) if matches!(expr.op, UnaryOp::Ref | UnaryOp::MutRef) => {
                self.infer_expr_ty(Some(&expr.expr))
            }
            Expr::Call(expr)
                if call_callee_name(&expr.callee)
                    .is_some_and(|name| self.enum_variants.contains_key(name)) =>
            {
                call_callee_name(&expr.callee)
                    .and_then(|name| self.enum_variants.get(name))
                    .map(|variant| IrValueTy::Enum(variant.enum_name.clone()))
            }
            Expr::Binary(expr) => {
                let left = self.infer_expr_ty(Some(&expr.left));
                let right = self.infer_expr_ty(Some(&expr.right));
                if expr.op == BinaryOp::Add
                    && (left.as_ref().is_some_and(is_string_ty)
                        || right.as_ref().is_some_and(is_string_ty))
                {
                    Some(IrValueTy::OwnedString)
                } else if left == Some(IrValueTy::Float64) || right == Some(IrValueTy::Float64) {
                    Some(IrValueTy::Float64)
                } else if matches!(
                    expr.op,
                    BinaryOp::Add
                        | BinaryOp::Sub
                        | BinaryOp::Mul
                        | BinaryOp::Div
                        | BinaryOp::Rem
                        | BinaryOp::BitAnd
                        | BinaryOp::BitOr
                        | BinaryOp::BitXor
                        | BinaryOp::Shl
                        | BinaryOp::Shr
                ) {
                    Some(IrValueTy::Int(IrIntTy::Int))
                } else {
                    None
                }
            }
            Expr::Index(expr) => {
                let target_ty = self.infer_expr_ty(Some(&expr.target));
                if target_ty.as_ref().is_some_and(is_byte_slice_ir_ty) {
                    Some(IrValueTy::Int(IrIntTy::Int))
                } else {
                    None
                }
            }
            Expr::Call(expr) if matches!(expr.callee.as_ref(), Expr::Ident(name) if name == "raw_write") => {
                Some(IrValueTy::Int(IrIntTy::Int))
            }
            Expr::Call(expr) if matches!(expr.callee.as_ref(), Expr::Ident(name) if name == "raw_alloc") => {
                Some(IrValueTy::Int(IrIntTy::Int))
            }
            Expr::Call(expr) if matches!(expr.callee.as_ref(), Expr::Ident(name) if name == "raw_load8" || name == "raw_load64" || name == "raw_string_ptr" || name == "raw_string_clone_ptr" || name == "raw_function_ptr" || name == "raw_float_to_int") => {
                Some(IrValueTy::Int(IrIntTy::Int))
            }
            Expr::Call(expr) if matches!(expr.callee.as_ref(), Expr::Ident(name) if name == "raw_set_nonblocking") => {
                Some(IrValueTy::Int(IrIntTy::Int))
            }
            Expr::Call(expr) if matches!(expr.callee.as_ref(), Expr::Ident(name) if name == "raw_thread_spawn" || name == "raw_thread_join") => {
                Some(IrValueTy::Int(IrIntTy::Int))
            }
            Expr::Call(expr) if matches!(expr.callee.as_ref(), Expr::Ident(name) if name == "raw_mem_alloc_count" || name == "raw_mem_free_count" || name == "raw_mem_live_bytes" || name == "raw_mem_high_water_bytes") => {
                Some(IrValueTy::Int(IrIntTy::Int))
            }
            Expr::Call(expr) if matches!(expr.callee.as_ref(), Expr::Ident(name) if name == "raw_strlen") => {
                Some(IrValueTy::Int(IrIntTy::Int))
            }
            Expr::Call(expr) if matches!(expr.callee.as_ref(), Expr::Ident(name) if name == "raw_string_concat" || name == "raw_int_to_string" || name == "raw_alloc_string") => {
                Some(IrValueTy::OwnedString)
            }
            Expr::Call(expr) if matches!(expr.callee.as_ref(), Expr::Ident(name) if name == "raw_string_from_ptr") => {
                Some(IrValueTy::String)
            }
            Expr::Call(expr) if matches!(expr.callee.as_ref(), Expr::Ident(name) if name == "raw_function_from_ptr") => {
                Some(IrValueTy::Function {
                    params: Vec::new(),
                    ret: Box::new(IrValueTy::Unit),
                    is_async: false,
                })
            }
            Expr::Call(expr) if matches!(expr.callee.as_ref(), Expr::Ident(name) if name == "raw_function_from_ptr_int") => {
                Some(IrValueTy::Function {
                    params: vec![IrValueTy::Int(IrIntTy::Int)],
                    ret: Box::new(IrValueTy::Unit),
                    is_async: false,
                })
            }
            Expr::Call(expr) if matches!(expr.callee.as_ref(), Expr::Ident(name) if name == "raw_function_from_ptr_handler") => {
                Some(IrValueTy::Function {
                    params: vec![IrValueTy::Int(IrIntTy::Int), IrValueTy::String],
                    ret: Box::new(IrValueTy::Int(IrIntTy::Int)),
                    is_async: false,
                })
            }
            Expr::Call(expr) if matches!(expr.callee.as_ref(), Expr::Ident(name) if name == "raw_function_from_ptr_request_handler") => {
                Some(IrValueTy::Function {
                    params: vec![
                        IrValueTy::Int(IrIntTy::Int),
                        IrValueTy::Struct("http__Request".to_string()),
                        IrValueTy::String,
                    ],
                    ret: Box::new(IrValueTy::Int(IrIntTy::Int)),
                    is_async: false,
                })
            }
            Expr::Call(expr) if matches!(expr.callee.as_ref(), Expr::Ident(name) if name == "raw_function_from_ptr_response_handler") => {
                Some(IrValueTy::Function {
                    params: vec![
                        IrValueTy::Int(IrIntTy::Int),
                        IrValueTy::Struct("http__Request".to_string()),
                        IrValueTy::String,
                    ],
                    ret: Box::new(IrValueTy::Struct("http__Response".to_string())),
                    is_async: false,
                })
            }
            Expr::Call(expr) => call_callee_name(&expr.callee)
                .and_then(|name| {
                    self.specialize_call_callee(name, expr)
                        .or_else(|| Some(name.to_string()))
                })
                .and_then(|name| self.function_sigs.get(&name))
                .map(|sig| sig.ret.clone()),
            Expr::Member(expr) => self.member_ty(expr),
            Expr::Block(block) => self.infer_block_ty(block),
            Expr::If(expr) => self.infer_if_ty(expr),
            Expr::Match(expr) => expr
                .arms
                .iter()
                .find_map(|arm| self.infer_expr_ty(Some(&arm.body))),
            _ => None,
        }
    }

    fn infer_if_ty(&self, expr: &IfExpr) -> Option<IrValueTy> {
        let then_ty = self.infer_block_ty(&expr.then_branch)?;
        let else_ty = expr
            .else_branch
            .as_ref()
            .and_then(|branch| self.infer_block_ty(branch))?;
        if then_ty == else_ty {
            Some(then_ty)
        } else {
            None
        }
    }

    fn infer_block_ty(&self, block: &Block) -> Option<IrValueTy> {
        let mut child_locals = self.local_types.clone();
        for stmt in &block.statements {
            match &stmt.data {
                StmtData::Let(stmt) => {
                    let ty = stmt
                        .ty_expr
                        .as_ref()
                        .map(|ty| {
                            ir_local_ty_from_type(ty, self.enum_names, self.generic_struct_names)
                        })
                        .or_else(|| {
                            self.infer_expr_ty_with_locals(stmt.value.as_ref(), &child_locals)
                        })
                        .unwrap_or(IrValueTy::Unknown);
                    child_locals.insert(stmt.name.clone(), ty);
                }
                StmtData::Expr(expr) => {
                    return self.infer_expr_ty_with_locals(Some(expr), &child_locals)
                }
                _ => {}
            }
        }
        None
    }

    fn infer_expr_ty_with_locals(
        &self,
        expr: Option<&Expr>,
        locals: &HashMap<String, IrValueTy>,
    ) -> Option<IrValueTy> {
        let expr = expr?;
        match expr {
            Expr::Ident(name) => locals
                .get(name)
                .cloned()
                .or_else(|| self.infer_expr_ty(Some(expr))),
            Expr::Unary(expr) if matches!(expr.op, UnaryOp::Ref | UnaryOp::MutRef) => {
                self.infer_expr_ty_with_locals(Some(&expr.expr), locals)
            }
            Expr::Binary(expr) => {
                let left = self.infer_expr_ty_with_locals(Some(&expr.left), locals);
                let right = self.infer_expr_ty_with_locals(Some(&expr.right), locals);
                if expr.op == BinaryOp::Add
                    && (left.as_ref().is_some_and(is_string_ty)
                        || right.as_ref().is_some_and(is_string_ty))
                {
                    Some(IrValueTy::OwnedString)
                } else if left == Some(IrValueTy::Float64) || right == Some(IrValueTy::Float64) {
                    Some(IrValueTy::Float64)
                } else if matches!(
                    expr.op,
                    BinaryOp::Add
                        | BinaryOp::Sub
                        | BinaryOp::Mul
                        | BinaryOp::Div
                        | BinaryOp::Rem
                        | BinaryOp::BitAnd
                        | BinaryOp::BitOr
                        | BinaryOp::BitXor
                        | BinaryOp::Shl
                        | BinaryOp::Shr
                ) {
                    Some(IrValueTy::Int(IrIntTy::Int))
                } else {
                    None
                }
            }
            Expr::Index(expr) => {
                let target_ty = self.infer_expr_ty_with_locals(Some(&expr.target), locals);
                if target_ty.as_ref().is_some_and(is_byte_slice_ir_ty) {
                    Some(IrValueTy::Int(IrIntTy::Int))
                } else {
                    None
                }
            }
            Expr::Block(block) => {
                let mut child_locals = self.local_types.clone();
                child_locals.extend(locals.clone());
                self.infer_block_ty_with_locals(block, &child_locals)
            }
            Expr::If(expr) => self.infer_if_ty_with_locals(expr, locals),
            Expr::Match(expr) => {
                let mut arms = expr.arms.iter();
                let first = arms.next()?;
                let first_ty = self.infer_expr_ty_with_locals(Some(&first.body), locals)?;
                if arms.all(|arm| {
                    self.infer_expr_ty_with_locals(Some(&arm.body), locals)
                        .as_ref()
                        == Some(&first_ty)
                }) {
                    Some(first_ty)
                } else {
                    None
                }
            }
            _ => self.infer_expr_ty(Some(expr)),
        }
    }

    fn infer_if_ty_with_locals(
        &self,
        expr: &IfExpr,
        locals: &HashMap<String, IrValueTy>,
    ) -> Option<IrValueTy> {
        let then_ty = self.infer_block_ty_with_locals(&expr.then_branch, locals)?;
        let else_ty = expr
            .else_branch
            .as_ref()
            .and_then(|branch| self.infer_block_ty_with_locals(branch, locals))?;
        if then_ty == else_ty {
            Some(then_ty)
        } else {
            None
        }
    }

    fn infer_block_ty_with_locals(
        &self,
        block: &Block,
        locals: &HashMap<String, IrValueTy>,
    ) -> Option<IrValueTy> {
        let mut child_locals = locals.clone();
        for stmt in &block.statements {
            match &stmt.data {
                StmtData::Let(stmt) => {
                    let ty = stmt
                        .ty_expr
                        .as_ref()
                        .map(|ty| {
                            ir_local_ty_from_type(ty, self.enum_names, self.generic_struct_names)
                        })
                        .or_else(|| {
                            self.infer_expr_ty_with_locals(stmt.value.as_ref(), &child_locals)
                        })
                        .unwrap_or(IrValueTy::Unknown);
                    child_locals.insert(stmt.name.clone(), ty);
                }
                StmtData::Expr(expr) => {
                    return self.infer_expr_ty_with_locals(Some(expr), &child_locals);
                }
                _ => {}
            }
        }
        None
    }

    fn member_ty(&self, expr: &MemberExpr) -> Option<IrValueTy> {
        let (base, field) = member_field_names(expr)?;
        match self.local_types.get(&base)? {
            IrValueTy::Struct(struct_name) => self.struct_fields.get(struct_name)?.get(&field),
            IrValueTy::Vec(_) => self
                .struct_fields
                .get("vec__Vec")
                .or_else(|| self.struct_fields.get("Vec"))?
                .get(&field),
            _ => return None,
        }
        .cloned()
    }

    fn specialize_call_callee(&self, name: &str, expr: &CallExpr) -> Option<String> {
        self.specialize_call_callee_with_channel_type_arg(name, expr, None)
    }

    fn specialize_call_callee_with_channel_type_arg(
        &self,
        name: &str,
        expr: &CallExpr,
        expected_channel_type_arg: Option<&str>,
    ) -> Option<String> {
        let inferred_type_arg = if expr.type_args.is_none() {
            self.infer_channel_call_type_arg(name, expr, expected_channel_type_arg)
        } else {
            None
        };
        specialize_channel_callee(
            name,
            expr.type_args.as_deref().or(inferred_type_arg.as_deref()),
        )
        .or_else(|| self.specialize_box_call_callee(name, expr))
        .or_else(|| self.specialize_map_call_callee(name, expr))
        .or_else(|| self.specialize_vec_call_callee(name, expr))
    }

    fn infer_channel_call_type_arg(
        &self,
        name: &str,
        expr: &CallExpr,
        expected_channel_type_arg: Option<&str>,
    ) -> Option<String> {
        match channel_generic_method(name)? {
            "new" => expected_channel_type_arg.map(str::to_string),
            "send" => expr
                .args
                .get(1)
                .and_then(|arg| self.infer_expr_ty(Some(arg)))
                .and_then(channel_type_arg_from_ir_ty),
            "clone" | "recv" | "close" | "destroy" => expr
                .args
                .first()
                .and_then(|arg| self.channel_type_arg_from_expr(arg))
                .or_else(|| expected_channel_type_arg.map(str::to_string)),
            _ => None,
        }
    }

    fn channel_type_arg_from_expr(&self, expr: &Expr) -> Option<String> {
        match expr {
            Expr::Ident(name) => self.channel_local_type_args.get(name).cloned(),
            Expr::Unary(expr) if matches!(expr.op, UnaryOp::Ref | UnaryOp::MutRef) => {
                self.channel_type_arg_from_expr(&expr.expr)
            }
            Expr::Mut(expr) => self.channel_type_arg_from_expr(expr),
            _ => None,
        }
    }

    fn specialize_box_call_callee(&self, name: &str, expr: &CallExpr) -> Option<String> {
        let is_box_member = matches!(
            expr.callee.as_ref(),
            Expr::Member(MemberExpr { target, .. })
                if matches!(target.as_ref(), Expr::Ident(target) if target == "box")
        );
        let method = box_generic_method(name, is_box_member)?;
        let type_arg = expr
            .type_args
            .as_deref()
            .map(str::to_string)
            .or_else(|| self.infer_box_call_type_arg(method, expr))?;
        specialize_box_callee(name, &type_arg)
    }

    fn infer_box_call_type_arg(&self, method: &str, expr: &CallExpr) -> Option<String> {
        match method {
            "new" => expr
                .args
                .first()
                .and_then(|arg| self.infer_expr_ty(Some(arg)))
                .and_then(box_type_arg_from_ir_ty),
            "take" | "destroy" => expr
                .args
                .first()
                .and_then(|arg| self.box_type_arg_from_expr(arg)),
            _ => None,
        }
    }

    fn box_type_arg_from_expr(&self, expr: &Expr) -> Option<String> {
        match expr {
            Expr::Ident(name) => self.box_local_type_args.get(name).cloned(),
            Expr::Unary(expr) if matches!(expr.op, UnaryOp::Ref | UnaryOp::MutRef) => {
                self.box_type_arg_from_expr(&expr.expr)
            }
            Expr::Mut(expr) => self.box_type_arg_from_expr(expr),
            _ => None,
        }
    }

    fn specialize_map_call_callee(&self, name: &str, expr: &CallExpr) -> Option<String> {
        let is_map_member = matches!(
            expr.callee.as_ref(),
            Expr::Member(MemberExpr { target, .. })
                if matches!(target.as_ref(), Expr::Ident(target) if target == "map")
        );
        let method = map_generic_method(name, is_map_member)?;
        let type_arg = expr
            .type_args
            .as_deref()
            .map(str::to_string)
            .or_else(|| self.infer_map_call_type_arg(method, expr))?;
        specialize_map_callee(name, &type_arg)
    }

    fn infer_map_call_type_arg(&self, method: &str, expr: &CallExpr) -> Option<String> {
        match method {
            "length" | "put" | "get" | "destroy" => expr
                .args
                .first()
                .and_then(|arg| self.map_type_arg_from_expr(arg)),
            "new" => None,
            _ => None,
        }
    }

    fn map_type_arg_from_expr(&self, expr: &Expr) -> Option<String> {
        match expr {
            Expr::Ident(name) => self.map_local_type_args.get(name).cloned(),
            Expr::Unary(expr) if matches!(expr.op, UnaryOp::Ref | UnaryOp::MutRef) => {
                self.map_type_arg_from_expr(&expr.expr)
            }
            Expr::Mut(expr) => self.map_type_arg_from_expr(expr),
            _ => None,
        }
    }

    fn specialize_vec_call_callee(&self, name: &str, expr: &CallExpr) -> Option<String> {
        let is_vec_member = matches!(
            expr.callee.as_ref(),
            Expr::Member(MemberExpr { target, .. })
                if matches!(target.as_ref(), Expr::Ident(target) if target == "vec")
        );
        let method = vec_generic_method(name, is_vec_member)?;
        let type_arg = expr
            .type_args
            .as_deref()
            .map(str::to_string)
            .or_else(|| self.infer_vec_call_type_arg(method, expr))?;
        specialize_vec_callee(name, &type_arg)
    }

    fn infer_vec_call_type_arg(&self, method: &str, expr: &CallExpr) -> Option<String> {
        match method {
            "data" | "length" | "capacity" | "push" | "get" | "destroy" => expr
                .args
                .first()
                .and_then(|arg| self.vec_type_arg_from_expr(arg)),
            "new" => None,
            _ => None,
        }
    }

    fn vec_type_arg_from_expr(&self, expr: &Expr) -> Option<String> {
        match expr {
            Expr::Ident(name) => self.vec_local_type_args.get(name).cloned(),
            Expr::Unary(expr) if matches!(expr.op, UnaryOp::Ref | UnaryOp::MutRef) => {
                self.vec_type_arg_from_expr(&expr.expr)
            }
            Expr::Mut(expr) => self.vec_type_arg_from_expr(expr),
            _ => None,
        }
    }

    fn lower_assignment(&mut self, expr: &Expr) -> bool {
        let Expr::Binary(expr) = expr else {
            return false;
        };
        if !expr.op.is_assignment() {
            return false;
        }
        let right = if let Some(value_op) = expr.op.compound_value_op() {
            Expr::Binary(BinaryExpr {
                op: value_op,
                left: expr.left.clone(),
                right: expr.right.clone(),
            })
        } else {
            (*expr.right).clone()
        };
        if let Expr::Member(member) = expr.left.as_ref() {
            if let Some((base, field)) = member_field_names(member) {
                if let Some(field_ty) = self.member_ty(member) {
                    if let Some(value) = self.lower_value_expr_with_hint(&right, Some(&field_ty)) {
                        self.push(IrInstruction::AssignField { base, field, value });
                        return true;
                    }
                }
            }
            self.lower_expr(&expr.left);
            self.lower_expr(&expr.right);
            return true;
        }
        let Some(local) = assign_target_name(&expr.left) else {
            self.lower_expr(&expr.left);
            self.lower_expr(&expr.right);
            return true;
        };
        if let Some(value) =
            self.lower_value_expr_with_hint(&right, self.local_types.get(&local).cloned().as_ref())
        {
            match value {
                IrValueExpr::Int(value) => self.push(IrInstruction::AssignInt { local, value }),
                IrValueExpr::Float(value) => self.push(IrInstruction::AssignFloat { local, value }),
                IrValueExpr::Bool(value) => self.push(IrInstruction::AssignBool { local, value }),
                IrValueExpr::String(value) => {
                    self.assign_heap_value_with_cleanup(local, IrValueExpr::String(value))
                }
                IrValueExpr::Struct(value) => {
                    self.assign_heap_value_with_cleanup(local, IrValueExpr::Struct(value))
                }
                IrValueExpr::Enum(value) => {
                    self.assign_heap_value_with_cleanup(local, IrValueExpr::Enum(value))
                }
                IrValueExpr::Function(value) => {
                    self.push(IrInstruction::AssignFunction { local, value })
                }
            }
        } else {
            self.lower_expr(&right);
        }
        true
    }

    fn assign_heap_value_with_cleanup(&mut self, local: String, value: IrValueExpr) {
        let local_ty = self
            .local_types
            .get(&local)
            .cloned()
            .unwrap_or(IrValueTy::Unknown);
        if !is_heap_owned_ty(&local_ty) || self.consumed_locals.contains(&local) {
            self.assign_value(local, value);
            return;
        }

        let temp = self.next_assignment_temp();
        self.assign_value(temp.clone(), value);
        self.push(IrInstruction::Let {
            local: temp.clone(),
        });
        self.locals.push(temp.clone());
        self.local_types.insert(temp.clone(), local_ty.clone());
        self.consumed_locals.insert(temp.clone());
        self.push(IrInstruction::Drop {
            local: local.clone(),
            ty: local_ty,
        });
        self.assign_value(local, local_value_expr(&temp, self.local_types.get(&temp)));
    }

    fn assign_value(&mut self, local: String, value: IrValueExpr) {
        match value {
            IrValueExpr::Int(value) => self.push(IrInstruction::AssignInt { local, value }),
            IrValueExpr::Float(value) => self.push(IrInstruction::AssignFloat { local, value }),
            IrValueExpr::Bool(value) => self.push(IrInstruction::AssignBool { local, value }),
            IrValueExpr::String(value) => self.push(IrInstruction::AssignString { local, value }),
            IrValueExpr::Struct(value) => self.push(IrInstruction::AssignStruct { local, value }),
            IrValueExpr::Enum(value) => self.push(IrInstruction::AssignEnum { local, value }),
            IrValueExpr::Function(value) => {
                self.push(IrInstruction::AssignFunction { local, value })
            }
        }
    }

    fn lower_try(&mut self, expr: &Expr) {
        self.lower_expr(expr);
        let ok_block = self.new_block();
        let error_block = self.new_block();
        let continue_block = self.new_block();
        self.push(IrInstruction::Try {
            ok_block,
            error_block,
        });
        self.set_terminator(IrTerminator::Branch {
            condition: None,
            then_block: ok_block,
            else_block: error_block,
        });

        self.current = error_block;
        self.append_return_drops(&HashSet::new());
        self.set_terminator(IrTerminator::Return);

        self.current = ok_block;
        self.set_terminator(IrTerminator::Jump {
            target: continue_block,
        });

        self.current = continue_block;
    }

    fn lower_try_assignment(&mut self, local: &str, expr: &Expr) -> bool {
        let Some(IrValueTy::Enum(operand_enum)) = self.infer_expr_ty(Some(expr)) else {
            self.lower_try(expr);
            return true;
        };
        let Some((success_variant, failure_variant)) = self.try_variants(&operand_enum) else {
            self.lower_try(expr);
            return true;
        };
        let Some(value) = self.lower_enum_expr(expr) else {
            self.lower_try(expr);
            return true;
        };
        let Some(success_payload_ty) = success_variant.payload_tys.first().cloned() else {
            self.lower_try(expr);
            return true;
        };

        let operand_local = self.next_assignment_temp();
        self.push(IrInstruction::AssignEnum {
            local: operand_local.clone(),
            value,
        });
        self.push(IrInstruction::Let {
            local: operand_local.clone(),
        });
        self.local_types
            .insert(operand_local.clone(), IrValueTy::Enum(operand_enum));
        self.locals.push(operand_local.clone());

        let ok_block = self.new_block();
        let error_block = self.new_block();
        let continue_block = self.new_block();
        self.push(IrInstruction::Try {
            ok_block,
            error_block,
        });
        self.set_terminator(IrTerminator::Branch {
            condition: Some(IrBoolExpr::Compare {
                op: IrCompareOp::Eq,
                left: Box::new(IrIntExpr::EnumTag(Box::new(IrEnumExpr::Local(
                    operand_local.clone(),
                )))),
                right: Box::new(IrIntExpr::Const(success_variant.tag)),
            }),
            then_block: ok_block,
            else_block: error_block,
        });

        self.current = error_block;
        self.lower_try_error_return(&operand_local, &failure_variant);

        self.current = ok_block;
        self.push(IrInstruction::BindEnumPayload {
            local: local.to_string(),
            value: IrEnumExpr::Local(operand_local),
            payload_index: 0,
            payload_tys: success_variant.payload_tys.clone(),
            payload_ty: success_payload_ty,
        });
        self.set_terminator(IrTerminator::Jump {
            target: continue_block,
        });

        self.current = continue_block;
        true
    }

    fn lower_try_error_return(&mut self, operand_local: &str, failure_variant: &EnumVariantSig) {
        let IrValueTy::Enum(return_enum) = self.return_type.clone() else {
            self.append_return_drops(&HashSet::new());
            self.set_terminator(IrTerminator::Return);
            return;
        };
        let Some(return_variants) = self.enum_variants_by_enum.get(&return_enum) else {
            self.append_return_drops(&HashSet::new());
            self.set_terminator(IrTerminator::Return);
            return;
        };

        if let Some(return_err) = return_variants.get("Err") {
            let Some(payload_ty) = failure_variant.payload_tys.first().cloned() else {
                self.append_return_drops(&HashSet::new());
                self.set_terminator(IrTerminator::Return);
                return;
            };
            let error_local = self.next_assignment_temp();
            self.push(IrInstruction::BindEnumPayload {
                local: error_local.clone(),
                value: IrEnumExpr::Local(operand_local.to_string()),
                payload_index: 0,
                payload_tys: failure_variant.payload_tys.clone(),
                payload_ty: payload_ty.clone(),
            });
            self.push(IrInstruction::Let {
                local: error_local.clone(),
            });
            self.local_types
                .insert(error_local.clone(), payload_ty.clone());
            self.locals.push(error_local.clone());
            let referenced = HashSet::from([operand_local.to_string(), error_local.clone()]);
            self.append_return_drops(&referenced);
            self.push(IrInstruction::ReturnEnum {
                value: IrEnumExpr::Construct {
                    enum_name: return_enum,
                    variant: "Err".to_string(),
                    tag: return_err.tag,
                    payloads: vec![local_value_expr(&error_local, Some(&payload_ty))],
                },
            });
            self.set_terminator(IrTerminator::Return);
            return;
        }

        if let Some(return_none) = return_variants.get("None") {
            let referenced = HashSet::from([operand_local.to_string()]);
            self.append_return_drops(&referenced);
            self.push(IrInstruction::ReturnEnum {
                value: IrEnumExpr::Construct {
                    enum_name: return_enum,
                    variant: "None".to_string(),
                    tag: return_none.tag,
                    payloads: Vec::new(),
                },
            });
            self.set_terminator(IrTerminator::Return);
            return;
        }

        self.append_return_drops(&HashSet::new());
        self.set_terminator(IrTerminator::Return);
    }

    fn infer_try_unwrap_ty(&self, expr: &Expr) -> Option<IrValueTy> {
        let IrValueTy::Enum(enum_name) = self.infer_expr_ty(Some(expr))? else {
            return None;
        };
        let (success, _) = self.try_variants(&enum_name)?;
        success.payload_tys.first().cloned()
    }

    fn try_variants(&self, enum_name: &str) -> Option<(EnumVariantSig, EnumVariantSig)> {
        let variants = self.enum_variants_by_enum.get(enum_name)?;
        if let (Some(ok), Some(err)) = (variants.get("Ok"), variants.get("Err")) {
            return Some((ok.clone(), err.clone()));
        }
        if let (Some(some), Some(none)) = (variants.get("Some"), variants.get("None")) {
            return Some((some.clone(), none.clone()));
        }
        None
    }

    fn lower_await(&mut self, expr: &Expr) {
        self.lower_expr(expr);
        self.record_suspend_point();
    }

    fn record_suspend_point(&mut self) {
        let point = self.suspend_points;
        self.suspend_points += 1;
        self.push(IrInstruction::Suspend { point });
        for local in self.live_borrows.clone() {
            self.push(IrInstruction::BorrowAcrossSuspend { local });
        }
    }

    fn lower_closure(&mut self, expr: &ClosureExpr) {
        if expr.is_async {
            self.push(IrInstruction::FutureState {
                captures: closure_captures(expr, &self.locals),
            });
        }
        self.lower_block(&expr.body);
    }

    fn lower_non_capturing_closure_function(
        &mut self,
        expr: &ClosureExpr,
    ) -> Option<IrFunctionExpr> {
        if expr.is_async || !closure_captures(expr, &self.locals).is_empty() {
            return None;
        }
        let index = self.closure_index;
        self.closure_index += 1;
        let name = format!("__closure_{}_{}", self.function_name, index);
        let param_types = expr
            .params
            .iter()
            .map(|param| {
                param
                    .ty_expr
                    .as_ref()
                    .map(|ty| ir_param_ty_from_type(ty, self.enum_names, self.generic_struct_names))
                    .unwrap_or(IrValueTy::Unknown)
            })
            .collect::<Vec<_>>();
        let return_type = expr
            .return_type_expr
            .as_ref()
            .map(|ty| ir_return_ty_from_type(ty, self.enum_names, self.generic_struct_names))
            .unwrap_or(IrValueTy::Unit);
        let mut lowerer = FunctionLowerer {
            blocks: vec![IrBlock {
                id: 0,
                instructions: Vec::new(),
                terminator: IrTerminator::Return,
            }],
            current: 0,
            loop_stack: Vec::new(),
            locals: expr
                .params
                .iter()
                .map(|param| normalize_param_name(&param.name))
                .collect(),
            local_types: expr
                .params
                .iter()
                .zip(param_types.iter())
                .map(|(param, ty)| (normalize_param_name(&param.name), ty.clone()))
                .collect(),
            channel_local_type_args: expr
                .params
                .iter()
                .filter_map(|param| {
                    let name = normalize_param_name(&param.name);
                    let type_arg = param
                        .ty_expr
                        .as_ref()
                        .and_then(channel_type_arg_from_type_expr)?;
                    Some((name, type_arg))
                })
                .collect(),
            box_local_type_args: expr
                .params
                .iter()
                .filter_map(|param| {
                    let name = normalize_param_name(&param.name);
                    let type_arg = param
                        .ty_expr
                        .as_ref()
                        .and_then(box_type_arg_from_type_expr)?;
                    Some((name, type_arg))
                })
                .collect(),
            map_local_type_args: expr
                .params
                .iter()
                .filter_map(|param| {
                    let name = normalize_param_name(&param.name);
                    let type_arg = param
                        .ty_expr
                        .as_ref()
                        .and_then(map_type_arg_from_type_expr)?;
                    Some((name, type_arg))
                })
                .collect(),
            vec_local_type_args: expr
                .params
                .iter()
                .filter_map(|param| {
                    let name = normalize_param_name(&param.name);
                    let type_arg = param
                        .ty_expr
                        .as_ref()
                        .and_then(vec_type_arg_from_type_expr)?;
                    Some((name, type_arg))
                })
                .collect(),
            function_sigs: self.function_sigs,
            struct_fields: self.struct_fields,
            enum_variants: self.enum_variants,
            enum_variants_by_enum: self.enum_variants_by_enum,
            enum_names: self.enum_names,
            generic_struct_names: self.generic_struct_names,
            return_type: return_type.clone(),
            live_borrows: Vec::new(),
            suspend_points: 0,
            module: self.module,
            function_name: name.clone(),
            closure_index: 0,
            generated_functions: Vec::new(),
            return_temp_index: 0,
            assignment_temp_index: 0,
            consumed_locals: HashSet::new(),
            return_drop_locals: Vec::new(),
            scope_stack: Vec::new(),
        };
        lowerer.lower_block(&expr.body);
        let nested = lowerer.generated_functions;
        self.generated_functions.extend(nested);
        self.generated_functions.push(IrFunction {
            module: self.module,
            name: name.clone(),
            is_async: false,
            future_state: None,
            params: expr
                .params
                .iter()
                .map(|param| normalize_param_name(&param.name))
                .collect(),
            param_types,
            return_type,
            blocks: lowerer.blocks,
        });
        Some(IrFunctionExpr::Named(name))
    }

    fn infer_struct_literal_ty(&self, expr: &StructExpr) -> IrValueTy {
        if !self.generic_struct_names.contains(&expr.name) {
            return IrValueTy::Struct(expr.name.clone());
        }
        let Some(fields) = self.struct_fields.get(&expr.name) else {
            return IrValueTy::Struct(expr.name.clone());
        };
        let args = expr
            .fields
            .iter()
            .filter_map(|field| {
                let IrValueTy::Struct(generic) = fields.get(&field.name)? else {
                    return None;
                };
                if generic.len() != 1 || !generic.chars().all(|ch| ch.is_ascii_uppercase()) {
                    return None;
                }
                self.infer_expr_ty(field.value.as_ref())
            })
            .collect::<Vec<_>>();
        if args.is_empty() {
            return IrValueTy::Struct(expr.name.clone());
        }
        IrValueTy::Struct(format!(
            "{}<{}>",
            expr.name,
            args.iter().map(ir_ty_name).collect::<Vec<_>>().join(",")
        ))
    }

    fn return_expr_needs_temp_before_drops(&self, referenced: &HashSet<String>) -> bool {
        let scoped_drop_needs_temp = self
            .scope_stack
            .iter()
            .skip(1)
            .flat_map(|scope| scope.iter())
            .any(|local| {
                referenced.contains(local)
                    && self.locals.contains(local)
                    && self.local_types.get(local).is_some_and(is_heap_owned_ty)
                    && self
                        .local_types
                        .get(local)
                        .is_some_and(return_referenced_local_needs_temp)
            });
        scoped_drop_needs_temp
            || self.return_drop_locals.iter().any(|local| {
                referenced.contains(local)
                    && self.locals.contains(local)
                    && self.local_types.get(local).is_some_and(is_heap_owned_ty)
                    && self
                        .local_types
                        .get(local)
                        .is_some_and(return_referenced_local_needs_temp)
            })
    }

    fn scalar_return_expr_needs_temp_before_drops(&self, referenced: &HashSet<String>) -> bool {
        let scoped_drop_needs_temp = self
            .scope_stack
            .iter()
            .skip(1)
            .flat_map(|scope| scope.iter())
            .any(|local| {
                referenced.contains(local)
                    && self.locals.contains(local)
                    && self
                        .local_types
                        .get(local)
                        .is_some_and(|ty| self.scalar_return_referenced_local_needs_temp(ty))
            });
        scoped_drop_needs_temp
            || self.return_drop_locals.iter().any(|local| {
                referenced.contains(local)
                    && self.locals.contains(local)
                    && self
                        .local_types
                        .get(local)
                        .is_some_and(|ty| self.scalar_return_referenced_local_needs_temp(ty))
            })
    }

    fn scalar_return_referenced_local_needs_temp(&self, ty: &IrValueTy) -> bool {
        match ty {
            IrValueTy::OwnedString
            | IrValueTy::Boxed(_)
            | IrValueTy::Vec(_)
            | IrValueTy::Enum(_) => true,
            IrValueTy::Struct(name) => {
                is_unique_resource_name(name)
                    || self.struct_fields.get(name).is_some_and(|fields| {
                        fields.values().any(|field_ty| {
                            matches!(
                                field_ty,
                                IrValueTy::String
                                    | IrValueTy::BorrowedString
                                    | IrValueTy::OwnedString
                            ) || is_heap_owned_ty(field_ty)
                        })
                    })
            }
            _ => false,
        }
    }

    fn next_return_temp(&mut self) -> String {
        let local = format!("__return{}", self.return_temp_index);
        self.return_temp_index += 1;
        local
    }

    fn next_assignment_temp(&mut self) -> String {
        let local = format!("__assign{}", self.assignment_temp_index);
        self.assignment_temp_index += 1;
        local
    }

    fn append_drops(&mut self, drops: &[String]) {
        for local in drops {
            let ty = self
                .local_types
                .get(local)
                .cloned()
                .unwrap_or(IrValueTy::Unknown);
            if is_heap_owned_ty(&ty)
                && (!self.consumed_locals.contains(local) || is_shared_handle_ty(&ty))
            {
                self.push(IrInstruction::Drop {
                    local: local.clone(),
                    ty,
                });
            }
        }
    }

    fn append_scope_exit_drops(&mut self, referenced: &HashSet<String>) {
        let mut drops = Vec::new();
        for local in self
            .scope_stack
            .iter()
            .skip(1)
            .rev()
            .flat_map(|scope| scope.iter().rev())
        {
            if drops.contains(local) {
                continue;
            }
            if self.consumed_locals.contains(local) || referenced.contains(local) {
                continue;
            }
            if self.local_types.get(local).is_some_and(is_heap_owned_ty) {
                drops.push(local.clone());
            }
        }
        self.append_drops(&drops);
    }

    fn append_return_drops(&mut self, referenced: &HashSet<String>) -> Vec<String> {
        let mut drops = Vec::new();
        for local in self
            .scope_stack
            .iter()
            .skip(1)
            .rev()
            .flat_map(|scope| scope.iter().rev())
        {
            if drops.contains(local) {
                continue;
            }
            if self.consumed_locals.contains(local) || referenced.contains(local) {
                continue;
            }
            if self.local_types.get(local).is_some_and(is_heap_owned_ty) {
                drops.push(local.clone());
            }
        }
        for local in self.return_drop_locals.iter().rev() {
            if drops.contains(local) {
                continue;
            }
            if !self.locals.contains(local)
                || self.consumed_locals.contains(local)
                || referenced.contains(local)
            {
                continue;
            }
            drops.push(local.clone());
        }
        self.append_drops(&drops);
        drops
    }

    fn push(&mut self, instruction: IrInstruction) {
        self.blocks[self.current].instructions.push(instruction);
    }

    fn set_terminator(&mut self, terminator: IrTerminator) {
        self.blocks[self.current].terminator = terminator;
    }

    fn new_block(&mut self) -> usize {
        let id = self.blocks.len();
        self.blocks.push(IrBlock {
            id,
            instructions: Vec::new(),
            terminator: IrTerminator::Return,
        });
        id
    }

    fn current_block_returns(&self) -> bool {
        self.blocks[self.current]
            .instructions
            .last()
            .is_some_and(|instruction| {
                matches!(
                    instruction,
                    IrInstruction::ReturnUnit
                        | IrInstruction::ReturnInt { .. }
                        | IrInstruction::ReturnFloat { .. }
                        | IrInstruction::ReturnBool { .. }
                        | IrInstruction::ReturnString { .. }
                        | IrInstruction::ReturnStruct { .. }
                        | IrInstruction::ReturnEnum { .. }
                        | IrInstruction::ReturnEnumMatch { .. }
                        | IrInstruction::ReturnFunction { .. }
                )
            })
    }

    fn lower_bool_expr(&mut self, expr: &Expr) -> Option<IrBoolExpr> {
        match expr {
            Expr::Literal(Literal::Bool(value)) => Some(IrBoolExpr::Const(*value)),
            Expr::Await(expr) => {
                self.record_suspend_point();
                self.lower_bool_expr(expr)
            }
            Expr::Ident(name) => Some(IrBoolExpr::Local(name.clone())),
            Expr::Member(expr) => {
                member_field_names(expr).map(|(base, field)| IrBoolExpr::Field { base, field })
            }
            Expr::Unary(expr) if matches!(expr.op, UnaryOp::Not) => {
                Some(IrBoolExpr::Not(Box::new(self.lower_bool_expr(&expr.expr)?)))
            }
            Expr::Call(expr) => {
                let callee = call_callee_name(&expr.callee)?;
                let callee = self
                    .specialize_call_callee(callee, expr)
                    .unwrap_or_else(|| callee.to_string());
                let sig = self.function_sigs.get(&callee)?;
                if sig.ret != IrValueTy::Bool {
                    return None;
                }
                Some(IrBoolExpr::Call {
                    callee,
                    args: self.lower_value_args(&expr.args, &sig.params)?,
                })
            }
            Expr::Binary(expr) => {
                if expr.op == BinaryOp::BoolAnd {
                    return Some(IrBoolExpr::And(
                        Box::new(self.lower_bool_expr(&expr.left)?),
                        Box::new(self.lower_bool_expr(&expr.right)?),
                    ));
                }
                if expr.op == BinaryOp::BoolOr {
                    return Some(IrBoolExpr::Or(
                        Box::new(self.lower_bool_expr(&expr.left)?),
                        Box::new(self.lower_bool_expr(&expr.right)?),
                    ));
                }
                let op = match expr.op {
                    BinaryOp::Eq => IrCompareOp::Eq,
                    BinaryOp::NotEq => IrCompareOp::NotEq,
                    BinaryOp::Lt => IrCompareOp::Lt,
                    BinaryOp::Le => IrCompareOp::Le,
                    BinaryOp::Gt => IrCompareOp::Gt,
                    BinaryOp::Ge => IrCompareOp::Ge,
                    _ => return None,
                };
                let left_ty = self.infer_expr_ty(Some(&expr.left));
                let right_ty = self.infer_expr_ty(Some(&expr.right));
                if matches!(op, IrCompareOp::Eq | IrCompareOp::NotEq)
                    && left_ty == Some(IrValueTy::Bool)
                    && right_ty == Some(IrValueTy::Bool)
                {
                    return Some(IrBoolExpr::BoolCompare {
                        op,
                        left: Box::new(self.lower_bool_expr(&expr.left)?),
                        right: Box::new(self.lower_bool_expr(&expr.right)?),
                    });
                }
                if matches!(op, IrCompareOp::Eq | IrCompareOp::NotEq)
                    && left_ty.as_ref().is_some_and(is_string_ty)
                    && right_ty.as_ref().is_some_and(is_string_ty)
                {
                    return Some(IrBoolExpr::StringCompare {
                        op,
                        left: Box::new(self.lower_string_expr(&expr.left)?),
                        right: Box::new(self.lower_string_expr(&expr.right)?),
                    });
                }
                if left_ty == Some(IrValueTy::Float64) || right_ty == Some(IrValueTy::Float64) {
                    return Some(IrBoolExpr::FloatCompare {
                        op,
                        left: Box::new(self.lower_float_expr(&expr.left)?),
                        right: Box::new(self.lower_float_expr(&expr.right)?),
                    });
                }
                Some(IrBoolExpr::Compare {
                    op,
                    left: Box::new(self.lower_int_expr(&expr.left)?),
                    right: Box::new(self.lower_int_expr(&expr.right)?),
                })
            }
            _ => None,
        }
    }

    fn lower_float_expr(&mut self, expr: &Expr) -> Option<IrFloatExpr> {
        match expr {
            Expr::Literal(Literal::Float(value)) => Some(IrFloatExpr::Const(value.clone())),
            Expr::Await(expr) => {
                self.record_suspend_point();
                self.lower_float_expr(expr)
            }
            Expr::Unary(expr) if matches!(expr.op, UnaryOp::Neg) => Some(IrFloatExpr::Binary {
                op: IrFloatBinaryOp::Sub,
                left: Box::new(IrFloatExpr::Const("0.0".to_string())),
                right: Box::new(self.lower_float_expr(&expr.expr)?),
            }),
            Expr::Ident(name) => match self.local_types.get(name) {
                Some(IrValueTy::Int(_)) => Some(IrFloatExpr::IntToFloat(Box::new(
                    IrIntExpr::Local(name.clone()),
                ))),
                _ => Some(IrFloatExpr::Local(name.clone())),
            },
            Expr::Member(expr) => {
                let (base, field) = member_field_names(expr)?;
                if matches!(self.member_ty(expr), Some(IrValueTy::Int(_))) {
                    Some(IrFloatExpr::IntToFloat(Box::new(IrIntExpr::Field {
                        base,
                        field,
                    })))
                } else {
                    Some(IrFloatExpr::Field { base, field })
                }
            }
            Expr::Binary(expr) => {
                let op = match expr.op {
                    BinaryOp::Add => IrFloatBinaryOp::Add,
                    BinaryOp::Sub => IrFloatBinaryOp::Sub,
                    BinaryOp::Mul => IrFloatBinaryOp::Mul,
                    BinaryOp::Div => IrFloatBinaryOp::Div,
                    _ => return None,
                };
                Some(IrFloatExpr::Binary {
                    op,
                    left: Box::new(self.lower_float_expr(&expr.left)?),
                    right: Box::new(self.lower_float_expr(&expr.right)?),
                })
            }
            Expr::Call(expr) => {
                let callee = call_callee_name(&expr.callee)?;
                let callee = self
                    .specialize_call_callee(callee, expr)
                    .unwrap_or_else(|| callee.to_string());
                let sig = self.function_sigs.get(&callee)?;
                if sig.ret != IrValueTy::Float64 {
                    return None;
                }
                Some(IrFloatExpr::Call {
                    callee,
                    args: self.lower_value_args(&expr.args, &sig.params)?,
                })
            }
            _ => self
                .lower_int_expr(expr)
                .map(|expr| IrFloatExpr::IntToFloat(Box::new(expr))),
        }
    }

    fn lower_int_expr(&mut self, expr: &Expr) -> Option<IrIntExpr> {
        match expr {
            Expr::Literal(Literal::Int(value)) => value.parse().ok().map(IrIntExpr::Const),
            Expr::Await(expr) => {
                self.record_suspend_point();
                self.lower_int_expr(expr)
            }
            Expr::Unary(expr) if matches!(expr.op, UnaryOp::Neg) => Some(IrIntExpr::Binary {
                op: IrIntBinaryOp::Sub,
                left: Box::new(IrIntExpr::Const(0)),
                right: Box::new(self.lower_int_expr(&expr.expr)?),
            }),
            Expr::Ident(name) => Some(IrIntExpr::Local(name.clone())),
            Expr::Member(expr) => {
                member_field_names(expr).map(|(base, field)| IrIntExpr::Field { base, field })
            }
            Expr::Binary(expr) => {
                let op = match expr.op {
                    BinaryOp::Add => IrIntBinaryOp::Add,
                    BinaryOp::Sub => IrIntBinaryOp::Sub,
                    BinaryOp::Mul => IrIntBinaryOp::Mul,
                    BinaryOp::Div => IrIntBinaryOp::Div,
                    BinaryOp::Rem => IrIntBinaryOp::Rem,
                    BinaryOp::BitAnd => IrIntBinaryOp::BitAnd,
                    BinaryOp::BitOr => IrIntBinaryOp::BitOr,
                    BinaryOp::BitXor => IrIntBinaryOp::BitXor,
                    BinaryOp::Shl => IrIntBinaryOp::Shl,
                    BinaryOp::Shr => IrIntBinaryOp::Shr,
                    _ => return None,
                };
                Some(IrIntExpr::Binary {
                    op,
                    left: Box::new(self.lower_int_expr(&expr.left)?),
                    right: Box::new(self.lower_int_expr(&expr.right)?),
                })
            }
            Expr::Index(expr)
                if self
                    .infer_expr_ty(Some(&expr.target))
                    .as_ref()
                    .is_some_and(is_byte_slice_ir_ty) =>
            {
                Some(IrIntExpr::Call {
                    callee: "slice__get".to_string(),
                    args: vec![
                        IrValueExpr::Struct(self.lower_struct_expr(&expr.target)?),
                        IrValueExpr::Int(self.lower_int_expr(&expr.index)?),
                    ],
                })
            }
            Expr::Call(expr) => {
                if matches!(expr.callee.as_ref(), Expr::Ident(name) if name == "raw_alloc") {
                    return match expr.args.as_slice() {
                        [size] => Some(IrIntExpr::RawAlloc {
                            size: Box::new(self.lower_int_expr(size)?),
                        }),
                        _ => None,
                    };
                }
                if matches!(expr.callee.as_ref(), Expr::Ident(name) if name == "raw_load8") {
                    return match expr.args.as_slice() {
                        [ptr, offset] => Some(IrIntExpr::RawLoad8 {
                            ptr: Box::new(self.lower_int_expr(ptr)?),
                            offset: Box::new(self.lower_int_expr(offset)?),
                        }),
                        _ => None,
                    };
                }
                if matches!(expr.callee.as_ref(), Expr::Ident(name) if name == "raw_load64") {
                    return match expr.args.as_slice() {
                        [ptr, offset] => Some(IrIntExpr::RawLoad64 {
                            ptr: Box::new(self.lower_int_expr(ptr)?),
                            offset: Box::new(self.lower_int_expr(offset)?),
                        }),
                        _ => None,
                    };
                }
                if matches!(expr.callee.as_ref(), Expr::Ident(name) if name == "raw_string_ptr") {
                    return match expr.args.as_slice() {
                        [value] => Some(IrIntExpr::StringPtr(Box::new(
                            self.lower_string_expr(value)?,
                        ))),
                        _ => None,
                    };
                }
                if matches!(expr.callee.as_ref(), Expr::Ident(name) if name == "raw_string_clone_ptr")
                {
                    return match expr.args.as_slice() {
                        [value] => Some(IrIntExpr::StringPtr(Box::new(IrStringExpr::Concat {
                            left: Box::new(self.lower_string_expr(value)?),
                            right: Box::new(IrStringExpr::Literal("".to_string())),
                        }))),
                        _ => None,
                    };
                }
                if matches!(expr.callee.as_ref(), Expr::Ident(name) if name == "raw_function_ptr") {
                    return match expr.args.as_slice() {
                        [value] => Some(IrIntExpr::FunctionPtr(self.lower_function_expr(value)?)),
                        _ => None,
                    };
                }
                if matches!(expr.callee.as_ref(), Expr::Ident(name) if name == "raw_float_to_int") {
                    return match expr.args.as_slice() {
                        [value] => Some(IrIntExpr::FloatToInt(Box::new(
                            self.lower_float_expr(value)?,
                        ))),
                        _ => None,
                    };
                }
                if matches!(expr.callee.as_ref(), Expr::Ident(name) if name == "raw_set_nonblocking")
                {
                    return match expr.args.as_slice() {
                        [fd] => Some(IrIntExpr::RawSetNonblocking {
                            fd: Box::new(self.lower_int_expr(fd)?),
                        }),
                        _ => None,
                    };
                }
                if matches!(expr.callee.as_ref(), Expr::Ident(name) if name == "raw_thread_spawn") {
                    return match expr.args.as_slice() {
                        [task] => {
                            let (task, captures) = self.lower_thread_spawn_task(task)?;
                            Some(IrIntExpr::RawThreadSpawn { task, captures })
                        }
                        _ => None,
                    };
                }
                if matches!(expr.callee.as_ref(), Expr::Ident(name) if name == "raw_thread_join") {
                    return match expr.args.as_slice() {
                        [handle] => Some(IrIntExpr::RawThreadJoin {
                            handle: Box::new(self.lower_int_expr(handle)?),
                        }),
                        _ => None,
                    };
                }
                if matches!(expr.callee.as_ref(), Expr::Ident(name) if name == "raw_strlen") {
                    return match expr.args.as_slice() {
                        [value] => Some(IrIntExpr::StringLen(Box::new(
                            self.lower_string_expr(value)?,
                        ))),
                        _ => None,
                    };
                }
                if matches!(expr.callee.as_ref(), Expr::Ident(name) if name == "raw_mem_alloc_count")
                {
                    return matches!(expr.args.as_slice(), [])
                        .then_some(IrIntExpr::RawMemAllocCount);
                }
                if matches!(expr.callee.as_ref(), Expr::Ident(name) if name == "raw_mem_free_count")
                {
                    return matches!(expr.args.as_slice(), [])
                        .then_some(IrIntExpr::RawMemFreeCount);
                }
                if matches!(expr.callee.as_ref(), Expr::Ident(name) if name == "raw_mem_live_bytes")
                {
                    return matches!(expr.args.as_slice(), [])
                        .then_some(IrIntExpr::RawMemLiveBytes);
                }
                if matches!(expr.callee.as_ref(), Expr::Ident(name) if name == "raw_mem_high_water_bytes")
                {
                    return matches!(expr.args.as_slice(), [])
                        .then_some(IrIntExpr::RawMemHighWaterBytes);
                }
                if matches!(expr.callee.as_ref(), Expr::Ident(name) if name == "raw_write") {
                    return match expr.args.as_slice() {
                        [fd, text] => Some(IrIntExpr::RawWrite {
                            fd: Box::new(self.lower_int_expr(fd)?),
                            text: Box::new(self.lower_string_expr(text)?),
                        }),
                        _ => None,
                    };
                }
                let function_value_ty = match expr.callee.as_ref() {
                    Expr::Ident(callee_name)
                        if matches!(
                            self.local_types.get(callee_name),
                            Some(IrValueTy::Function { .. })
                        ) =>
                    {
                        self.local_types.get(callee_name).cloned()
                    }
                    Expr::Member(_) => self.infer_expr_ty(Some(&expr.callee)),
                    _ => None,
                };
                if let Some(IrValueTy::Function { params, ret, .. }) = function_value_ty {
                    if matches!(ret.as_ref(), IrValueTy::Int(_)) {
                        let args = self.lower_value_args(&expr.args, &params)?;
                        return Some(IrIntExpr::IndirectCall {
                            callee: self.lower_function_expr(&expr.callee)?,
                            args,
                        });
                    }
                }
                let callee = call_callee_name(&expr.callee)?;
                let callee = self
                    .specialize_call_callee(callee, expr)
                    .unwrap_or_else(|| callee.to_string());
                if self.enum_variants.contains_key(&callee) {
                    return None;
                }
                let sig = self.function_sigs.get(&callee)?;
                if !matches!(sig.ret, IrValueTy::Int(_)) {
                    return None;
                }
                let args = self.lower_value_args(&expr.args, &sig.params)?;
                Some(IrIntExpr::Call { callee, args })
            }
            _ => None,
        }
    }
}

fn direct_callee_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Ident(name) => Some(name),
        Expr::Path(path) => path.last().map(String::as_str),
        _ => None,
    }
}

fn call_callee_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Member(member) => Some(member.member.as_str()),
        _ => direct_callee_name(expr),
    }
}

fn is_thread_spawn_callee(name: &str) -> bool {
    name == "spawn" || name.ends_with("__spawn")
}

fn specialize_channel_callee(name: &str, type_args: Option<&str>) -> Option<String> {
    let method = channel_generic_method(name)?;
    let suffix = match type_args.map(str::trim).filter(|value| !value.is_empty()) {
        Some("String") => "string",
        Some("Bool") => "bool",
        Some("Int") | None => "int",
        Some(value) if is_unit_function_type_arg(value) => "function",
        Some(_) => return None,
    };
    Some(replace_method_suffix(
        name,
        method,
        &format!("{method}_{suffix}"),
    ))
}

fn channel_type_arg_from_ir_ty(ty: IrValueTy) -> Option<String> {
    match ty {
        IrValueTy::String | IrValueTy::BorrowedString | IrValueTy::OwnedString => {
            Some("String".to_string())
        }
        IrValueTy::Bool => Some("Bool".to_string()),
        IrValueTy::Int(_) => Some("Int".to_string()),
        IrValueTy::Function {
            params,
            ret,
            is_async: false,
        } if params.is_empty() && matches!(ret.as_ref(), IrValueTy::Unit) => Some("fn".to_string()),
        _ => None,
    }
}

fn specialize_box_callee(name: &str, type_arg: &str) -> Option<String> {
    let method = box_generic_method(name, false)?;
    let suffix = match type_arg.trim() {
        "Int" | "" => "int",
        "String" => "string",
        _ => return None,
    };
    Some(replace_method_suffix(
        name,
        method,
        &format!("{method}_{suffix}"),
    ))
}

fn specialize_map_callee(name: &str, type_arg: &str) -> Option<String> {
    let method = map_generic_method(name, false)?;
    let suffix = match normalized_type_args(type_arg).as_deref() {
        Some("String,String") => "string_string",
        _ => return None,
    };
    Some(replace_method_suffix(
        name,
        method,
        &format!("{method}_{suffix}"),
    ))
}

fn map_generic_method(name: &str, is_map_member: bool) -> Option<&'static str> {
    ["new", "length", "put", "get", "destroy"]
        .into_iter()
        .find(|method| {
            (is_map_member && name == *method)
                || name == format!("map__{method}")
                || name.ends_with(&format!("__map__{method}"))
        })
}

fn box_generic_method(name: &str, is_box_member: bool) -> Option<&'static str> {
    ["new", "take", "destroy"].into_iter().find(|method| {
        (is_box_member && name == *method)
            || name == format!("box__{method}")
            || name.ends_with(&format!("__box__{method}"))
    })
}

fn box_type_arg_from_ir_ty(ty: IrValueTy) -> Option<String> {
    match ty {
        IrValueTy::Int(_) => Some("Int".to_string()),
        IrValueTy::String | IrValueTy::BorrowedString | IrValueTy::OwnedString => {
            Some("String".to_string())
        }
        _ => None,
    }
}

fn specialize_vec_callee(name: &str, type_arg: &str) -> Option<String> {
    let method = vec_generic_method(name, false)?;
    let normalized = normalized_type_args(type_arg).unwrap_or_else(|| type_arg.trim().to_string());
    let normalized = normalized.to_ascii_lowercase();
    let suffix = match normalized.as_str() {
        "int" | "" => "int",
        "string" => "string",
        "fn(int,&str)->int" => "handler",
        "fn(int,request,str)->int"
        | "fn(int,&request,&str)->int"
        | "fn(int,http,str)->int"
        | "fn(int,&http.request,&str)->int"
        | "fn(int,http__request,str)->int"
        | "fn(int,&http__request,&str)->int" => "request_handler",
        "fn(int,http.request,str)->http.response"
        | "fn(int,&http.request,&str)->http.response"
        | "fn(int,http__request,str)->http__response"
        | "fn(int,&http__request,&str)->http__response" => "response_handler",
        _ => return None,
    };
    Some(replace_method_suffix(
        name,
        method,
        &format!("{method}_{suffix}"),
    ))
}

fn vec_generic_method(name: &str, is_vec_member: bool) -> Option<&'static str> {
    [
        "new", "data", "length", "capacity", "push", "get", "destroy",
    ]
    .into_iter()
    .find(|method| {
        (is_vec_member && name == *method)
            || name == format!("vec__{method}")
            || name.ends_with(&format!("__vec__{method}"))
    })
}

fn vec_type_arg_from_type_expr(expr: &TypeExpr) -> Option<String> {
    match expr {
        TypeExpr::Generic { base, args } if is_vec_type_base(base) => {
            args.first().and_then(type_expr_payload_name)
        }
        _ => None,
    }
}

fn map_type_arg_from_type_expr(expr: &TypeExpr) -> Option<String> {
    match expr {
        TypeExpr::Generic { base, args } if is_map_type_base(base) => {
            let key = args.first().and_then(type_expr_payload_name)?;
            let value = args.get(1).and_then(type_expr_payload_name)?;
            Some(format!("{key},{value}"))
        }
        _ => None,
    }
}

fn is_map_type_base(expr: &TypeExpr) -> bool {
    type_base_name(expr).is_some_and(|name| name.ends_with("Map"))
}

fn is_vec_type_base(expr: &TypeExpr) -> bool {
    type_base_name(expr).is_some_and(|name| name.ends_with("Vec"))
}

fn box_type_arg_from_type_expr(expr: &TypeExpr) -> Option<String> {
    match expr {
        TypeExpr::Generic { base, args } if is_box_type_base(base) => {
            args.first().and_then(type_expr_payload_name)
        }
        _ => None,
    }
}

fn is_box_type_base(expr: &TypeExpr) -> bool {
    type_base_name(expr).is_some_and(|name| name.ends_with("Box"))
}

fn channel_type_arg_from_type_expr(expr: &TypeExpr) -> Option<String> {
    match expr {
        TypeExpr::Generic { base, args } if is_channel_type_base(base) => {
            args.first().and_then(type_expr_payload_name)
        }
        _ => None,
    }
}

fn is_channel_type_base(expr: &TypeExpr) -> bool {
    type_base_name(expr).is_some_and(|name| name.ends_with("Channel"))
}

fn type_expr_payload_name(expr: &TypeExpr) -> Option<String> {
    match expr {
        TypeExpr::Path(path) => path.first().cloned(),
        TypeExpr::Fn {
            is_async: false,
            params,
            return_type,
        } => Some(format!(
            "fn({})->{}",
            params
                .iter()
                .filter_map(type_expr_payload_name)
                .collect::<Vec<_>>()
                .join(","),
            type_expr_payload_name(return_type)?
        )),
        TypeExpr::Tuple(items) if items.is_empty() => Some("()".to_string()),
        TypeExpr::Ref { inner, .. } | TypeExpr::Mut(inner) => type_expr_payload_name(inner),
        _ => None,
    }
}

fn type_base_name(expr: &TypeExpr) -> Option<String> {
    match expr {
        TypeExpr::Path(path) => path.first().cloned(),
        TypeExpr::Generic { base, .. } => type_base_name(base),
        TypeExpr::Ref { inner, .. }
        | TypeExpr::RawPtr { inner, .. }
        | TypeExpr::Impl(inner)
        | TypeExpr::Mut(inner) => type_base_name(inner),
        _ => None,
    }
}

fn channel_generic_method(name: &str) -> Option<&'static str> {
    if !name.starts_with("channel__")
        && name != "new"
        && name != "clone"
        && name != "send"
        && name != "recv"
        && name != "close"
        && name != "destroy"
    {
        return None;
    }
    for method in ["new", "clone", "send", "recv", "close", "destroy"] {
        if name == method || name.ends_with(&format!("__{method}")) {
            return Some(method);
        }
    }
    None
}

fn replace_method_suffix(name: &str, method: &str, replacement: &str) -> String {
    if name == method {
        return replacement.to_string();
    }
    let prefix_len = name.len().saturating_sub(method.len());
    format!("{}{}", &name[..prefix_len], replacement)
}

fn normalized_type_args(value: &str) -> Option<String> {
    let parts = split_top_level_csv(value);
    if parts.is_empty() {
        None
    } else {
        Some(
            parts
                .into_iter()
                .map(|part| part.split_whitespace().collect::<String>())
                .collect::<Vec<_>>()
                .join(","),
        )
    }
}

fn is_unit_function_type_arg(value: &str) -> bool {
    let normalized = value.split_whitespace().collect::<String>().to_lowercase();
    normalized == "fn()->()" || normalized == "fn"
}

fn consumed_cleanup_arg_from_ir_int(value: &IrIntExpr) -> Option<String> {
    let IrIntExpr::Call { callee, args } = value else {
        return None;
    };
    if !is_consuming_cleanup_name(callee) {
        return None;
    }
    match args.first()? {
        IrValueExpr::Struct(IrStructExpr::Local(local)) => Some(local.clone()),
        _ => None,
    }
}

fn consumed_box_take_string_arg_from_ir_string(value: &IrStringExpr) -> Option<String> {
    let IrStringExpr::Call { callee, args } = value else {
        return None;
    };
    if callee != "box__take_string" {
        return None;
    }
    match args.first()? {
        IrValueExpr::Struct(IrStructExpr::Local(local)) => Some(local.clone()),
        _ => None,
    }
}

fn ownership_transfer_arg_indexes(name: &str) -> &'static [usize] {
    match name {
        "alloc_box__store_string" => &[1],
        "alloc_vec__store_string" => &[2],
        "vec__push_string" => &[1],
        "alloc_map__put_string_string"
        | "alloc__map__put_string_string"
        | "map__put_string_string" => &[2, 3],
        _ => &[],
    }
}

fn ownership_transfer_param_indexes(name: &str) -> &'static [usize] {
    ownership_transfer_arg_indexes(name)
}

fn consumed_cleanup_arg(call: &CallExpr) -> Option<String> {
    if !is_consuming_cleanup_callee(&call.callee) {
        return None;
    }
    match call.args.first()? {
        Expr::Ident(name) => Some(name.clone()),
        _ => None,
    }
}

fn is_consuming_cleanup_name(name: &str) -> bool {
    matches!(
        name,
        "buffer__destroy"
            | "buffer__finish"
            | "buffer__string_builder_destroy"
            | "buffer__string_builder_finish"
            | "buffer__byte_buffer_destroy"
            | "buffer__byte_buffer_finish"
            | "box__destroy_int"
            | "http__headers_destroy"
            | "http__request_destroy"
            | "http__response_destroy"
            | "map__destroy"
            | "map__destroy_string_string"
            | "net__tcp_listener_close"
            | "net__tcp_stream_close"
            | "task__destroy_queue"
            | "task__destroy_queue_int"
            | "vec__destroy"
            | "vec__destroy_int"
            | "vec__destroy_string"
    )
}

fn is_consuming_cleanup_callee(expr: &Expr) -> bool {
    direct_callee_name(expr).is_some_and(is_consuming_cleanup_name)
        || matches!(
            expr,
            Expr::Member(MemberExpr { target, member })
                if member == "destroy"
                    && matches!(target.as_ref(), Expr::Ident(name) if name == "channel")
        )
        || matches!(
            expr,
            Expr::Member(MemberExpr { target, member })
                if matches!(member.as_str(), "destroy" | "finish")
                    && matches!(target.as_ref(), Expr::Ident(name) if name == "buffer")
        )
        || matches!(
            expr,
            Expr::Member(MemberExpr { target, member })
                if member == "destroy"
                        && matches!(target.as_ref(), Expr::Ident(name) if name == "box")
        )
        || matches!(
            expr,
            Expr::Member(MemberExpr { target, member })
                if member == "destroy"
                        && matches!(target.as_ref(), Expr::Ident(name) if name == "vec")
        )
        || matches!(
            expr,
            Expr::Member(MemberExpr { target, member })
                if member == "destroy"
                        && matches!(target.as_ref(), Expr::Ident(name) if name == "map")
        )
        || matches!(
            expr,
            Expr::Member(MemberExpr { target, member })
                if matches!(member.as_str(), "destroy_queue" | "destroy_queue_int")
                    && matches!(target.as_ref(), Expr::Ident(name) if name == "task")
        )
        || matches!(
            expr,
            Expr::Member(MemberExpr { target, member })
                if matches!(member.as_str(), "tcp_listener_close" | "tcp_stream_close")
                    && matches!(target.as_ref(), Expr::Ident(name) if name == "net")
        )
}

fn referenced_locals(expr: &Expr) -> HashSet<String> {
    let mut locals = HashSet::new();
    collect_referenced_locals(expr, &mut locals);
    locals
}

fn collect_referenced_locals(expr: &Expr, locals: &mut HashSet<String>) {
    match expr {
        Expr::Ident(name) => {
            locals.insert(name.clone());
        }
        Expr::Unary(expr) => collect_referenced_locals(&expr.expr, locals),
        Expr::Mut(expr) | Expr::Await(expr) | Expr::Try(expr) => {
            collect_referenced_locals(expr, locals)
        }
        Expr::Binary(expr) => {
            collect_referenced_locals(&expr.left, locals);
            collect_referenced_locals(&expr.right, locals);
        }
        Expr::Index(expr) => {
            collect_referenced_locals(&expr.target, locals);
            collect_referenced_locals(&expr.index, locals);
        }
        Expr::Call(expr) => {
            collect_referenced_locals(&expr.callee, locals);
            for arg in &expr.args {
                collect_referenced_locals(arg, locals);
            }
        }
        Expr::Member(expr) => collect_referenced_locals(&expr.target, locals),
        Expr::Struct(expr) => {
            for field in &expr.fields {
                if let Some(value) = &field.value {
                    collect_referenced_locals(value, locals);
                }
            }
        }
        Expr::Object(expr) => {
            for field in &expr.fields {
                collect_referenced_locals(&field.value, locals);
            }
        }
        Expr::Closure(expr) => {
            for stmt in &expr.body.statements {
                collect_referenced_locals_in_stmt(stmt, locals);
            }
        }
        Expr::Match(expr) => {
            collect_referenced_locals(&expr.value, locals);
            for arm in &expr.arms {
                collect_referenced_locals(&arm.body, locals);
            }
        }
        Expr::If(expr) => {
            collect_referenced_locals(&expr.condition, locals);
            for stmt in &expr.then_branch.statements {
                collect_referenced_locals_in_stmt(stmt, locals);
            }
            if let Some(else_branch) = &expr.else_branch {
                for stmt in &else_branch.statements {
                    collect_referenced_locals_in_stmt(stmt, locals);
                }
            }
        }
        Expr::Block(block) => {
            for stmt in &block.statements {
                collect_referenced_locals_in_stmt(stmt, locals);
            }
        }
        Expr::Missing | Expr::Literal(_) | Expr::Path(_) | Expr::Raw(_) => {}
    }
}

fn collect_referenced_locals_in_stmt(stmt: &Stmt, locals: &mut HashSet<String>) {
    match &stmt.data {
        StmtData::Let(stmt) => {
            if let Some(value) = &stmt.value {
                collect_referenced_locals(value, locals);
            }
        }
        StmtData::Return(expr) | StmtData::Break(expr) => {
            if let Some(expr) = expr {
                collect_referenced_locals(expr, locals);
            }
        }
        StmtData::If(control) | StmtData::While(control) => {
            if let Some(condition) = &control.condition {
                collect_referenced_locals(condition, locals);
            }
            for stmt in &control.body.statements {
                collect_referenced_locals_in_stmt(stmt, locals);
            }
        }
        StmtData::Match(expr) => collect_referenced_locals(&Expr::Match(expr.clone()), locals),
        StmtData::For(stmt) => {
            collect_referenced_locals(&stmt.iterator, locals);
            for stmt in &stmt.body.statements {
                collect_referenced_locals_in_stmt(stmt, locals);
            }
        }
        StmtData::Loop(block) | StmtData::Unsafe(block) => {
            for stmt in &block.statements {
                collect_referenced_locals_in_stmt(stmt, locals);
            }
        }
        StmtData::Expr(expr) => collect_referenced_locals(expr, locals),
        StmtData::Continue | StmtData::Raw => {}
    }
}

fn assign_target_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Ident(name) => Some(name.clone()),
        _ => None,
    }
}

fn member_field_names(expr: &MemberExpr) -> Option<(String, String)> {
    let Expr::Ident(target) = expr.target.as_ref() else {
        return None;
    };
    Some((target.clone(), expr.member.clone()))
}

fn pattern_variant_name(pattern: &str) -> Option<String> {
    pattern
        .split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
        .find(|part| !part.is_empty())
        .filter(|part| {
            part.chars()
                .next()
                .is_some_and(|ch| ch.is_ascii_uppercase())
        })
        .map(str::to_string)
}

fn pattern_bindings(pattern: &str) -> Vec<String> {
    let Some(open) = pattern.find('(') else {
        return Vec::new();
    };
    let Some(close) = pattern.rfind(')') else {
        return Vec::new();
    };
    split_top_level_csv(&pattern[open + 1..close])
        .into_iter()
        .filter(|part| *part != "_")
        .filter(|part| {
            part.chars()
                .next()
                .is_some_and(|ch| ch.is_ascii_lowercase() || ch == '_')
        })
        .map(str::to_string)
        .collect()
}

fn split_top_level_csv(text: &str) -> Vec<&str> {
    let mut items = Vec::new();
    let mut start = 0usize;
    let mut depth = 0i32;
    for (index, ch) in text.char_indices() {
        match ch {
            '<' | '(' | '[' | '{' => depth += 1,
            '>' | ')' | ']' | '}' => depth -= 1,
            ',' if depth == 0 => {
                let item = text[start..index].trim();
                if !item.is_empty() {
                    items.push(item);
                }
                start = index + 1;
            }
            _ => {}
        }
    }
    let item = text[start..].trim();
    if !item.is_empty() {
        items.push(item);
    }
    items
}

fn ir_ty_from_payload_text(text: &str, enum_names: &HashSet<String>) -> IrValueTy {
    let base = text
        .split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
        .find(|part| !part.is_empty())
        .unwrap_or("");
    match base {
        "Bool" => IrValueTy::Bool,
        "Int" => IrValueTy::Int(IrIntTy::Int),
        "Int8" => IrValueTy::Int(IrIntTy::Int8),
        "Int16" => IrValueTy::Int(IrIntTy::Int16),
        "Int32" => IrValueTy::Int(IrIntTy::Int32),
        "Int64" => IrValueTy::Int(IrIntTy::Int64),
        "UInt" => IrValueTy::Int(IrIntTy::UInt),
        "UInt8" => IrValueTy::Int(IrIntTy::UInt8),
        "UInt16" => IrValueTy::Int(IrIntTy::UInt16),
        "UInt32" => IrValueTy::Int(IrIntTy::UInt32),
        "UInt64" => IrValueTy::Int(IrIntTy::UInt64),
        "Float64" => IrValueTy::Float64,
        "String" => IrValueTy::String,
        "Str" => IrValueTy::BorrowedString,
        name if name.len() == 1 && name.chars().all(|ch| ch.is_ascii_uppercase()) => {
            IrValueTy::Unknown
        }
        name if enum_names.contains(name) => IrValueTy::Enum(name.to_string()),
        name if !name.is_empty()
            && name
                .chars()
                .next()
                .is_some_and(|ch| ch.is_ascii_uppercase()) =>
        {
            IrValueTy::Struct(name.to_string())
        }
        _ => IrValueTy::Unknown,
    }
}

fn ir_ty_from_payload_text_with_substitutions(
    text: &str,
    enum_names: &HashSet<String>,
    substitutions: &HashMap<String, IrValueTy>,
) -> IrValueTy {
    let text = text.trim();
    let text = text
        .strip_prefix('&')
        .map(str::trim)
        .unwrap_or(text)
        .strip_prefix("mut")
        .map(str::trim)
        .unwrap_or(text);
    substitutions
        .get(text)
        .cloned()
        .unwrap_or_else(|| ir_ty_from_payload_text(text, enum_names))
}

fn parse_generic_params(generics: Option<&str>) -> Vec<String> {
    generics
        .map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|part| !part.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn generic_instance_name(
    base: &TypeExpr,
    args: &[TypeExpr],
    enum_names: &HashSet<String>,
    generic_struct_names: &HashSet<String>,
) -> String {
    let base_name = type_base_name(base).unwrap_or_else(|| "Anonymous".to_string());
    let args = args
        .iter()
        .map(|arg| ir_ty_name(&ir_ty_from_type(arg, enum_names, generic_struct_names)))
        .collect::<Vec<_>>()
        .join(",");
    format!("{base_name}<{args}>")
}

fn ir_ty_name(ty: &IrValueTy) -> String {
    match ty {
        IrValueTy::Unit => "()".to_string(),
        IrValueTy::Bool => "Bool".to_string(),
        IrValueTy::Int(kind) => format!("{kind:?}"),
        IrValueTy::Float64 => "Float64".to_string(),
        IrValueTy::String | IrValueTy::OwnedString => "String".to_string(),
        IrValueTy::BorrowedString => "Str".to_string(),
        IrValueTy::Struct(name) | IrValueTy::Enum(name) => name.clone(),
        IrValueTy::Boxed(inner) => format!("Box<{}>", ir_ty_name(inner)),
        IrValueTy::Vec(inner) => format!("Vec<{}>", ir_ty_name(inner)),
        IrValueTy::Function { .. } => "fn".to_string(),
        IrValueTy::Unknown => "Unknown".to_string(),
    }
}

fn ir_ty_from_type_with_substitutions(
    expr: &TypeExpr,
    enum_names: &HashSet<String>,
    generic_struct_names: &HashSet<String>,
    substitutions: &HashMap<String, IrValueTy>,
) -> IrValueTy {
    if let TypeExpr::Path(path) = expr {
        if let Some(name) = path.first() {
            if let Some(ty) = substitutions.get(name) {
                return ty.clone();
            }
        }
    }
    match expr {
        TypeExpr::Generic { base, args } if is_box_type_base(base) => args
            .first()
            .map(|arg| {
                IrValueTy::Boxed(Box::new(ir_ty_from_type_with_substitutions(
                    arg,
                    enum_names,
                    generic_struct_names,
                    substitutions,
                )))
            })
            .unwrap_or_else(|| {
                ir_ty_from_type_with_substitutions(
                    base,
                    enum_names,
                    generic_struct_names,
                    substitutions,
                )
            }),
        TypeExpr::Generic { base, args } if is_vec_type_base(base) => args
            .first()
            .map(|arg| {
                IrValueTy::Vec(Box::new(ir_ty_from_type_with_substitutions(
                    arg,
                    enum_names,
                    generic_struct_names,
                    substitutions,
                )))
            })
            .unwrap_or_else(|| {
                ir_ty_from_type_with_substitutions(
                    base,
                    enum_names,
                    generic_struct_names,
                    substitutions,
                )
            }),
        TypeExpr::Generic { base, args }
            if type_base_name(base)
                .as_deref()
                .is_some_and(|name| generic_struct_names.contains(name)) =>
        {
            IrValueTy::Struct(generic_instance_name(
                base,
                args,
                enum_names,
                generic_struct_names,
            ))
        }
        TypeExpr::Generic { base, .. } => ir_ty_from_type_with_substitutions(
            base,
            enum_names,
            generic_struct_names,
            substitutions,
        ),
        TypeExpr::Fn {
            params,
            return_type,
            is_async,
        } => IrValueTy::Function {
            params: params
                .iter()
                .map(|param| {
                    ir_param_ty_from_type_with_substitutions(
                        param,
                        enum_names,
                        generic_struct_names,
                        substitutions,
                    )
                })
                .collect(),
            ret: Box::new(ir_return_ty_from_type_with_substitutions(
                return_type,
                enum_names,
                generic_struct_names,
                substitutions,
            )),
            is_async: *is_async,
        },
        TypeExpr::Mut(inner) => ir_ty_from_type_with_substitutions(
            inner,
            enum_names,
            generic_struct_names,
            substitutions,
        ),
        TypeExpr::Ref { inner, .. } => ir_borrowed_ty_from_type_with_substitutions(
            inner,
            enum_names,
            generic_struct_names,
            substitutions,
        ),
        _ => ir_ty_from_type(expr, enum_names, generic_struct_names),
    }
}

fn ir_param_ty_from_type_with_substitutions(
    expr: &TypeExpr,
    enum_names: &HashSet<String>,
    generic_struct_names: &HashSet<String>,
    substitutions: &HashMap<String, IrValueTy>,
) -> IrValueTy {
    if is_by_value_string_type(expr) {
        IrValueTy::OwnedString
    } else {
        ir_ty_from_type_with_substitutions(expr, enum_names, generic_struct_names, substitutions)
    }
}

fn ir_return_ty_from_type_with_substitutions(
    expr: &TypeExpr,
    enum_names: &HashSet<String>,
    generic_struct_names: &HashSet<String>,
    substitutions: &HashMap<String, IrValueTy>,
) -> IrValueTy {
    if is_by_value_string_type(expr) {
        IrValueTy::OwnedString
    } else {
        ir_ty_from_type_with_substitutions(expr, enum_names, generic_struct_names, substitutions)
    }
}

fn ir_borrowed_ty_from_type_with_substitutions(
    expr: &TypeExpr,
    enum_names: &HashSet<String>,
    generic_struct_names: &HashSet<String>,
    substitutions: &HashMap<String, IrValueTy>,
) -> IrValueTy {
    match expr {
        TypeExpr::Path(path)
            if path
                .first()
                .is_some_and(|name| name == "String" || name == "Str") =>
        {
            IrValueTy::BorrowedString
        }
        TypeExpr::Mut(inner) => ir_borrowed_ty_from_type_with_substitutions(
            inner,
            enum_names,
            generic_struct_names,
            substitutions,
        ),
        _ => ir_ty_from_type_with_substitutions(
            expr,
            enum_names,
            generic_struct_names,
            substitutions,
        ),
    }
}

fn print_string_literal(expr: &CallExpr) -> Option<String> {
    if !matches!(expr.callee.as_ref(), Expr::Ident(name) if name == "print") {
        return None;
    }
    match expr.args.as_slice() {
        [Expr::Literal(Literal::String(value))] => Some(value.clone()),
        _ => None,
    }
}

fn normalize_param_name(name: &str) -> String {
    match name {
        "self" | "&self" | "&mut self" => "self".to_string(),
        _ => name.to_string(),
    }
}

fn closure_captures(expr: &ClosureExpr, locals: &[String]) -> Vec<String> {
    let params: Vec<_> = expr
        .params
        .iter()
        .map(|param| normalize_param_name(&param.name))
        .collect();
    let mut captures = Vec::new();
    collect_block_captures(&expr.body, locals, &params, &mut captures);
    captures.sort();
    captures.dedup();
    captures
}

fn collect_block_captures(
    block: &Block,
    locals: &[String],
    params: &[String],
    captures: &mut Vec<String>,
) {
    let mut local_params = params.to_vec();
    for stmt in &block.statements {
        if let StmtData::Let(stmt) = &stmt.data {
            local_params.push(stmt.name.clone());
        }
        collect_stmt_captures(stmt, locals, &local_params, captures);
    }
}

fn collect_stmt_captures(
    stmt: &Stmt,
    locals: &[String],
    params: &[String],
    captures: &mut Vec<String>,
) {
    match &stmt.data {
        StmtData::Let(stmt) => {
            if let Some(value) = &stmt.value {
                collect_expr_captures(value, locals, params, captures);
            }
        }
        StmtData::Return(expr) | StmtData::Break(expr) => {
            if let Some(expr) = expr {
                collect_expr_captures(expr, locals, params, captures);
            }
        }
        StmtData::Continue | StmtData::Raw => {}
        StmtData::If(control) | StmtData::While(control) => {
            if let Some(condition) = &control.condition {
                collect_expr_captures(condition, locals, params, captures);
            }
            collect_block_captures(&control.body, locals, params, captures);
        }
        StmtData::Match(expr) => collect_match_captures(expr, locals, params, captures),
        StmtData::For(stmt) => {
            collect_expr_captures(&stmt.iterator, locals, params, captures);
            collect_block_captures(&stmt.body, locals, params, captures);
        }
        StmtData::Loop(block) | StmtData::Unsafe(block) => {
            collect_block_captures(block, locals, params, captures);
        }
        StmtData::Expr(expr) => collect_expr_captures(expr, locals, params, captures),
    }
}

fn collect_expr_captures(
    expr: &Expr,
    locals: &[String],
    params: &[String],
    captures: &mut Vec<String>,
) {
    match expr {
        Expr::Missing | Expr::Path(_) | Expr::Literal(_) | Expr::Raw(_) => {}
        Expr::Ident(name) => {
            if locals.contains(name) && !params.contains(name) {
                captures.push(name.clone());
            }
        }
        Expr::Unary(expr) => collect_expr_captures(&expr.expr, locals, params, captures),
        Expr::Mut(expr) | Expr::Await(expr) | Expr::Try(expr) => {
            collect_expr_captures(expr, locals, params, captures);
        }
        Expr::Binary(expr) => {
            collect_expr_captures(&expr.left, locals, params, captures);
            collect_expr_captures(&expr.right, locals, params, captures);
        }
        Expr::Index(expr) => {
            collect_expr_captures(&expr.target, locals, params, captures);
            collect_expr_captures(&expr.index, locals, params, captures);
        }
        Expr::Call(expr) => {
            collect_expr_captures(&expr.callee, locals, params, captures);
            for arg in &expr.args {
                collect_expr_captures(arg, locals, params, captures);
            }
        }
        Expr::Member(expr) => collect_expr_captures(&expr.target, locals, params, captures),
        Expr::Struct(expr) => {
            for field in &expr.fields {
                if let Some(value) = &field.value {
                    collect_expr_captures(value, locals, params, captures);
                }
            }
        }
        Expr::Object(expr) => {
            for field in &expr.fields {
                collect_expr_captures(&field.value, locals, params, captures);
            }
        }
        Expr::Closure(_) => {}
        Expr::Match(expr) => collect_match_captures(expr, locals, params, captures),
        Expr::If(expr) => {
            collect_expr_captures(&expr.condition, locals, params, captures);
            collect_block_captures(&expr.then_branch, locals, params, captures);
            if let Some(else_branch) = &expr.else_branch {
                collect_block_captures(else_branch, locals, params, captures);
            }
        }
        Expr::Block(block) => collect_block_captures(block, locals, params, captures),
    }
}

fn collect_match_captures(
    expr: &MatchExpr,
    locals: &[String],
    params: &[String],
    captures: &mut Vec<String>,
) {
    collect_expr_captures(&expr.value, locals, params, captures);
    for arm in &expr.arms {
        collect_expr_captures(&arm.body, locals, params, captures);
    }
}

fn ir_ty_from_type(
    expr: &TypeExpr,
    enum_names: &HashSet<String>,
    generic_struct_names: &HashSet<String>,
) -> IrValueTy {
    match expr {
        TypeExpr::Path(path) => path
            .first()
            .map(|name| match name.as_str() {
                "Bool" => IrValueTy::Bool,
                "Int" => IrValueTy::Int(IrIntTy::Int),
                "Int8" => IrValueTy::Int(IrIntTy::Int8),
                "Int16" => IrValueTy::Int(IrIntTy::Int16),
                "Int32" => IrValueTy::Int(IrIntTy::Int32),
                "Int64" => IrValueTy::Int(IrIntTy::Int64),
                "UInt" => IrValueTy::Int(IrIntTy::UInt),
                "UInt8" => IrValueTy::Int(IrIntTy::UInt8),
                "UInt16" => IrValueTy::Int(IrIntTy::UInt16),
                "UInt32" => IrValueTy::Int(IrIntTy::UInt32),
                "UInt64" => IrValueTy::Int(IrIntTy::UInt64),
                "Float64" => IrValueTy::Float64,
                "String" => IrValueTy::String,
                "Str" => IrValueTy::BorrowedString,
                other if enum_names.contains(other) => IrValueTy::Enum(other.to_string()),
                other => IrValueTy::Struct(other.to_string()),
            })
            .unwrap_or(IrValueTy::Unknown),
        TypeExpr::Tuple(items) if items.is_empty() => IrValueTy::Unit,
        TypeExpr::Generic { base, args } if is_box_type_base(base) => args
            .first()
            .map(|arg| {
                IrValueTy::Boxed(Box::new(ir_ty_from_type(
                    arg,
                    enum_names,
                    generic_struct_names,
                )))
            })
            .unwrap_or_else(|| ir_ty_from_type(base, enum_names, generic_struct_names)),
        TypeExpr::Generic { base, args } if is_vec_type_base(base) => args
            .first()
            .map(|arg| {
                IrValueTy::Vec(Box::new(ir_ty_from_type(
                    arg,
                    enum_names,
                    generic_struct_names,
                )))
            })
            .unwrap_or_else(|| ir_ty_from_type(base, enum_names, generic_struct_names)),
        TypeExpr::Generic { base, args }
            if type_base_name(base)
                .as_deref()
                .is_some_and(|name| enum_names.contains(name)) =>
        {
            IrValueTy::Enum(generic_instance_name(
                base,
                args,
                enum_names,
                generic_struct_names,
            ))
        }
        TypeExpr::Generic { base, args }
            if type_base_name(base)
                .as_deref()
                .is_some_and(|name| generic_struct_names.contains(name)) =>
        {
            IrValueTy::Struct(generic_instance_name(
                base,
                args,
                enum_names,
                generic_struct_names,
            ))
        }
        TypeExpr::Generic { base, .. } => ir_ty_from_type(base, enum_names, generic_struct_names),
        TypeExpr::Fn {
            params,
            return_type,
            is_async,
        } => IrValueTy::Function {
            params: params
                .iter()
                .map(|param| ir_param_ty_from_type(param, enum_names, generic_struct_names))
                .collect(),
            ret: Box::new(ir_return_ty_from_type(
                return_type,
                enum_names,
                generic_struct_names,
            )),
            is_async: *is_async,
        },
        TypeExpr::Mut(inner) => ir_ty_from_type(inner, enum_names, generic_struct_names),
        TypeExpr::Ref { inner, .. } => {
            ir_borrowed_ty_from_type(inner, enum_names, generic_struct_names)
        }
        TypeExpr::RawPtr { .. } => IrValueTy::Int(IrIntTy::Int),
        TypeExpr::Missing | TypeExpr::Tuple(_) | TypeExpr::Impl(_) => IrValueTy::Unknown,
    }
}

fn ir_borrowed_ty_from_type(
    expr: &TypeExpr,
    enum_names: &HashSet<String>,
    generic_struct_names: &HashSet<String>,
) -> IrValueTy {
    match expr {
        TypeExpr::Path(path)
            if path
                .first()
                .is_some_and(|name| name == "String" || name == "Str") =>
        {
            IrValueTy::BorrowedString
        }
        TypeExpr::Mut(inner) => ir_borrowed_ty_from_type(inner, enum_names, generic_struct_names),
        _ => ir_ty_from_type(expr, enum_names, generic_struct_names),
    }
}
