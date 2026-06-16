use std::collections::{HashMap, HashSet};

use crate::ast::*;
use crate::hir::{HirFunction, HirModuleId, HirProgram};
use crate::semantics::Diagnostic;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeReport {
    pub checked_functions: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Ty {
    Unknown,
    Never,
    Unit,
    Bool,
    Int(IntKind),
    Float,
    Str,
    String,
    RawPtr {
        mutable: bool,
    },
    Fn {
        params: Vec<Ty>,
        ret: Box<Ty>,
        is_async: bool,
    },
    Generic {
        base: String,
        args: Vec<Ty>,
    },
    Named(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IntKind {
    Untyped,
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

#[derive(Debug, Clone)]
struct FunctionSig {
    generics: Vec<String>,
    params: Vec<Ty>,
    ret: Ty,
}

#[derive(Debug, Clone)]
struct StructSig {
    module: HirModuleId,
    generics: Vec<String>,
    fields: HashMap<String, FieldSig>,
}

#[derive(Debug, Clone)]
struct FieldSig {
    public: bool,
    ty: Ty,
}

#[derive(Debug, Clone)]
struct InterfaceSig {
    methods: HashMap<String, FunctionSig>,
}

#[derive(Debug, Clone)]
struct EnumSig {
    generics: Vec<String>,
    variants: HashMap<String, VariantSig>,
}

#[derive(Debug, Clone)]
struct VariantSig {
    enum_name: String,
    enum_generics: Vec<String>,
    params: Vec<Ty>,
}

pub fn type_check_program(program: &HirProgram) -> Result<TypeReport, Vec<Diagnostic>> {
    let mut checker = TypeChecker::new(program);
    checker.check_program(program);
    checker.finish()
}

struct TypeChecker {
    functions: HashMap<String, FunctionSig>,
    methods: HashMap<String, HashMap<String, FunctionSig>>,
    structs: HashMap<String, StructSig>,
    enums: HashMap<String, EnumSig>,
    enum_variants: HashMap<String, VariantSig>,
    interfaces: HashMap<String, InterfaceSig>,
    diagnostics: Vec<Diagnostic>,
    checked_functions: usize,
    return_tys: Vec<Ty>,
    current_module: HirModuleId,
}

impl TypeChecker {
    fn new(program: &HirProgram) -> Self {
        let mut functions = HashMap::new();
        for function in &program.functions {
            functions.insert(
                function.name.clone(),
                FunctionSig {
                    generics: parse_generic_params(function.generics.as_deref()),
                    params: function
                        .params
                        .iter()
                        .map(|param| {
                            param
                                .ty_expr
                                .as_ref()
                                .map(type_from_expr)
                                .unwrap_or(Ty::Unknown)
                        })
                        .collect(),
                    ret: function
                        .return_type
                        .as_ref()
                        .map(type_from_expr)
                        .unwrap_or(Ty::Unit),
                },
            );
        }
        for function in &program.extern_functions {
            functions.insert(
                function.name.clone(),
                FunctionSig {
                    generics: Vec::new(),
                    params: function
                        .params
                        .iter()
                        .map(|param| {
                            param
                                .ty_expr
                                .as_ref()
                                .map(type_from_expr)
                                .unwrap_or(Ty::Unknown)
                        })
                        .collect(),
                    ret: function
                        .return_type
                        .as_ref()
                        .map(type_from_expr)
                        .unwrap_or(Ty::Unit),
                },
            );
        }

        let mut methods: HashMap<String, HashMap<String, FunctionSig>> = HashMap::new();
        for implementation in &program.impls {
            for method in &implementation.methods {
                methods
                    .entry(implementation.target.clone())
                    .or_default()
                    .insert(method.name.clone(), function_item_sig(method));
            }
        }

        let mut structs = HashMap::new();
        for item in &program.structs {
            structs.insert(
                item.name.clone(),
                StructSig {
                    module: item.module,
                    generics: parse_generic_params(item.generics.as_deref()),
                    fields: item
                        .fields
                        .iter()
                        .map(|field| {
                            (
                                field.name.clone(),
                                FieldSig {
                                    public: field.public,
                                    ty: type_from_expr(&field.ty_expr),
                                },
                            )
                        })
                        .collect(),
                },
            );
        }

        let mut enums = HashMap::new();
        let mut enum_variants = HashMap::new();
        for item in &program.enums {
            let generics = parse_generic_params(item.generics.as_deref());
            let variants = item
                .variants
                .iter()
                .map(|variant| {
                    let sig = VariantSig {
                        enum_name: item.name.clone(),
                        enum_generics: generics.clone(),
                        params: variant_payload_tys(variant.payload.as_deref(), &generics),
                    };
                    enum_variants.insert(variant.name.clone(), sig.clone());
                    (variant.name.clone(), sig)
                })
                .collect();
            enums.insert(item.name.clone(), EnumSig { generics, variants });
        }

        let mut interfaces = HashMap::new();
        for item in &program.interfaces {
            interfaces.insert(
                item.name.clone(),
                InterfaceSig {
                    methods: item
                        .methods
                        .iter()
                        .map(|method| {
                            (
                                method.name.clone(),
                                FunctionSig {
                                    generics: Vec::new(),
                                    params: method
                                        .params
                                        .iter()
                                        .map(|param| {
                                            param
                                                .ty_expr
                                                .as_ref()
                                                .map(type_from_expr)
                                                .unwrap_or(Ty::Unknown)
                                        })
                                        .collect(),
                                    ret: method
                                        .return_type_expr
                                        .as_ref()
                                        .map(type_from_expr)
                                        .unwrap_or(Ty::Unit),
                                },
                            )
                        })
                        .collect(),
                },
            );
        }

        Self {
            functions,
            methods,
            structs,
            enums,
            enum_variants,
            interfaces,
            diagnostics: Vec::new(),
            checked_functions: 0,
            return_tys: Vec::new(),
            current_module: HirModuleId(0),
        }
    }

    fn check_program(&mut self, program: &HirProgram) {
        self.check_interface_impls(program);
        for function in &program.functions {
            self.check_function(function);
        }
    }

    fn check_interface_impls(&mut self, program: &HirProgram) {
        for implementation in &program.impls {
            let Some(interface_name) = &implementation.interface else {
                continue;
            };
            let Some(interface) = self.interfaces.get(interface_name).cloned() else {
                continue;
            };

            let methods: HashMap<_, _> = implementation
                .methods
                .iter()
                .map(|method| (method.name.clone(), method))
                .collect();

            for (required_name, required_sig) in interface.methods {
                let Some(actual) = methods.get(&required_name) else {
                    self.error(format!(
                        "missing method `{required_name}` for interface `{interface_name}`"
                    ));
                    continue;
                };

                let actual_params: Vec<_> = actual
                    .params
                    .iter()
                    .map(|param| {
                        param
                            .ty_expr
                            .as_ref()
                            .map(type_from_expr)
                            .unwrap_or(Ty::Unknown)
                    })
                    .collect();
                if actual_params.len() != required_sig.params.len() {
                    self.error(format!(
                        "method `{required_name}` parameter count mismatch: expected {}, got {}",
                        required_sig.params.len(),
                        actual_params.len()
                    ));
                } else {
                    for (idx, (expected, actual)) in required_sig
                        .params
                        .iter()
                        .zip(actual_params.iter())
                        .enumerate()
                    {
                        if !types_compatible_for_interface(expected, actual) {
                            self.error(format!(
                                "method `{required_name}` parameter {idx} type mismatch: expected {}, got {}",
                                display_ty(expected),
                                display_ty(actual)
                            ));
                        }
                    }
                }

                let actual_return = actual
                    .return_type_expr
                    .as_ref()
                    .map(type_from_expr)
                    .unwrap_or(Ty::Unit);
                if !types_compatible_for_interface(&required_sig.ret, &actual_return) {
                    self.error(format!(
                        "method `{required_name}` return type mismatch: expected {}, got {}",
                        display_ty(&required_sig.ret),
                        display_ty(&actual_return)
                    ));
                }
            }
        }
    }

    fn check_function(&mut self, function: &HirFunction) {
        self.checked_functions += 1;
        let previous_module = self.current_module;
        self.current_module = function.module;
        let expected_return = function
            .return_type
            .as_ref()
            .map(type_from_expr)
            .unwrap_or(Ty::Unit);
        let mut scope = HashMap::new();
        for param in &function.params {
            scope.insert(
                normalize_param_name(&param.name),
                param
                    .ty_expr
                    .as_ref()
                    .map(type_from_expr)
                    .unwrap_or(Ty::Unknown),
            );
        }
        self.return_tys.push(expected_return.clone());
        self.check_block(&function.body, &mut scope, &expected_return);
        self.return_tys.pop();
        self.current_module = previous_module;
    }

    fn check_block(
        &mut self,
        block: &Block,
        scope: &mut HashMap<String, Ty>,
        expected_return: &Ty,
    ) -> Ty {
        let mut last = Ty::Unit;
        for stmt in &block.statements {
            last = self.check_stmt(stmt, scope, expected_return);
        }
        last
    }

    fn check_child_block(
        &mut self,
        block: &Block,
        scope: &HashMap<String, Ty>,
        expected_return: &Ty,
    ) -> Ty {
        let mut child = scope.clone();
        self.check_block(block, &mut child, expected_return)
    }

    fn check_stmt(
        &mut self,
        stmt: &Stmt,
        scope: &mut HashMap<String, Ty>,
        expected_return: &Ty,
    ) -> Ty {
        match &stmt.data {
            StmtData::Let(stmt) => {
                let declared_ty = stmt.ty_expr.as_ref().map(type_from_expr);
                let declared_type_arg = stmt
                    .ty_expr
                    .as_ref()
                    .and_then(channel_type_arg_from_type_expr);
                let value_ty = stmt
                    .value
                    .as_ref()
                    .map(|value| {
                        self.infer_expr_with_expected(
                            value,
                            scope,
                            declared_ty.as_ref(),
                            declared_type_arg.as_deref(),
                        )
                    })
                    .unwrap_or(Ty::Unknown);
                if let Some(declared_ty) = declared_ty {
                    if let Some(value) = &stmt.value {
                        self.expect_expr_assignable(
                            &declared_ty,
                            &value_ty,
                            value,
                            "let type mismatch",
                        );
                    } else {
                        self.expect_assignable(&declared_ty, &value_ty, "let type mismatch");
                    }
                    scope.insert(stmt.name.clone(), declared_ty);
                } else {
                    scope.insert(stmt.name.clone(), default_local_ty(value_ty));
                }
                Ty::Unit
            }
            StmtData::Return(expr) => {
                let actual = expr
                    .as_ref()
                    .map(|expr| {
                        self.infer_expr_with_expected(expr, scope, Some(expected_return), None)
                    })
                    .unwrap_or(Ty::Unit);
                if let Some(expr) = expr {
                    self.expect_expr_assignable(
                        expected_return,
                        &actual,
                        expr,
                        "return type mismatch",
                    );
                } else {
                    self.expect_assignable(expected_return, &actual, "return type mismatch");
                }
                Ty::Never
            }
            StmtData::Break(expr) => expr
                .as_ref()
                .map(|expr| {
                    self.infer_expr(expr, scope);
                    Ty::Never
                })
                .unwrap_or(Ty::Never),
            StmtData::Continue => Ty::Never,
            StmtData::Expr(expr) => self.infer_expr(expr, scope),
            StmtData::If(control) | StmtData::While(control) => {
                if let Some(condition) = &control.condition {
                    let condition_ty = self.infer_expr(condition, scope);
                    self.expect_assignable(&Ty::Bool, &condition_ty, "condition type mismatch");
                }
                self.check_child_block(&control.body, scope, expected_return)
            }
            StmtData::Match(expr) => self.infer_match(expr, scope),
            StmtData::For(stmt) => {
                self.infer_expr(&stmt.iterator, scope);
                let mut child = scope.clone();
                for binding in pattern_bindings(&stmt.pattern) {
                    child.insert(binding, Ty::Unknown);
                }
                self.check_block(&stmt.body, &mut child, expected_return);
                Ty::Unit
            }
            StmtData::Loop(block) | StmtData::Unsafe(block) => {
                self.check_child_block(block, scope, expected_return)
            }
            StmtData::Raw => Ty::Unknown,
        }
    }

    fn infer_expr(&mut self, expr: &Expr, scope: &mut HashMap<String, Ty>) -> Ty {
        self.infer_expr_with_channel_type_arg(expr, scope, None)
    }

    fn infer_expr_with_channel_type_arg(
        &mut self,
        expr: &Expr,
        scope: &mut HashMap<String, Ty>,
        expected_channel_type_arg: Option<&str>,
    ) -> Ty {
        self.infer_expr_with_expected(expr, scope, None, expected_channel_type_arg)
    }

    fn infer_expr_with_expected(
        &mut self,
        expr: &Expr,
        scope: &mut HashMap<String, Ty>,
        expected_ty: Option<&Ty>,
        expected_channel_type_arg: Option<&str>,
    ) -> Ty {
        match expr {
            Expr::Missing | Expr::Raw(_) => Ty::Unknown,
            Expr::Ident(name) => scope
                .get(name)
                .cloned()
                .unwrap_or_else(|| self.functions.get(name).map(fn_ty).unwrap_or(Ty::Unknown)),
            Expr::Path(_) => Ty::Unknown,
            Expr::Literal(literal) => match literal {
                Literal::Int(_) => Ty::Int(IntKind::Untyped),
                Literal::Float(_) => Ty::Float,
                Literal::String(_) => Ty::Str,
                Literal::Char(_) => Ty::Named("Char".to_string()),
                Literal::Bool(_) => Ty::Bool,
                Literal::Unit => Ty::Unit,
            },
            Expr::Unary(expr) => self.infer_expr(&expr.expr, scope),
            Expr::Mut(expr) | Expr::Await(expr) => self.infer_expr(expr, scope),
            Expr::Try(expr) => self.infer_try_expr(expr, scope),
            Expr::Binary(expr) => {
                let left = self.infer_expr(&expr.left, scope);
                let right = if expr.op.is_assignment() {
                    self.infer_expr_with_expected(&expr.right, scope, Some(&left), None)
                } else {
                    self.infer_expr(&expr.right, scope)
                };
                match expr.op {
                    BinaryOp::Eq | BinaryOp::NotEq => {
                        if !equality_comparable(&left, &right) {
                            self.error(format!(
                                "equality comparison type mismatch: {} and {}",
                                display_ty(&left),
                                display_ty(&right)
                            ));
                        }
                        Ty::Bool
                    }
                    BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => {
                        if !ordered_comparable(&left, &right) {
                            self.error(format!(
                                "ordered comparison type mismatch: {} and {}",
                                display_ty(&left),
                                display_ty(&right)
                            ));
                        }
                        Ty::Bool
                    }
                    BinaryOp::Assign
                    | BinaryOp::AddAssign
                    | BinaryOp::SubAssign
                    | BinaryOp::MulAssign
                    | BinaryOp::DivAssign
                    | BinaryOp::RemAssign
                    | BinaryOp::BitAndAssign
                    | BinaryOp::BitOrAssign
                    | BinaryOp::ShlAssign
                    | BinaryOp::ShrAssign => {
                        self.expect_expr_assignable(
                            &left,
                            &right,
                            &expr.right,
                            "assignment type mismatch",
                        );
                        if let Some(value_op) = expr.op.compound_value_op() {
                            self.check_binary_operand_types(value_op, &left, &right);
                        }
                        Ty::Unit
                    }
                    BinaryOp::BoolAnd | BinaryOp::BoolOr => {
                        self.expect_assignable(&Ty::Bool, &left, "boolean operator type mismatch");
                        self.expect_assignable(&Ty::Bool, &right, "boolean operator type mismatch");
                        Ty::Bool
                    }
                    BinaryOp::BitAnd
                    | BinaryOp::BitOr
                    | BinaryOp::BitXor
                    | BinaryOp::Shl
                    | BinaryOp::Shr => {
                        self.check_binary_operand_types(expr.op, &left, &right);
                        Ty::Int(IntKind::Int)
                    }
                    BinaryOp::Add
                    | BinaryOp::Sub
                    | BinaryOp::Mul
                    | BinaryOp::Div
                    | BinaryOp::Rem => {
                        self.check_binary_operand_types(expr.op, &left, &right);
                        if expr.op == BinaryOp::Add
                            && (is_string_like_ty(&left) || is_string_like_ty(&right))
                        {
                            Ty::String
                        } else if matches!(left, Ty::Float) || matches!(right, Ty::Float) {
                            Ty::Float
                        } else {
                            Ty::Int(IntKind::Int)
                        }
                    }
                }
            }
            Expr::Index(expr) => {
                let target = self.infer_expr(&expr.target, scope);
                let index = self.infer_expr(&expr.index, scope);
                self.expect_assignable(&Ty::Int(IntKind::Int), &index, "slice index type mismatch");
                if is_byte_slice_ty(&target) {
                    Ty::Int(IntKind::Int)
                } else {
                    self.error(format!(
                        "indexing is not supported for `{}`",
                        display_ty(&target)
                    ));
                    Ty::Unknown
                }
            }
            Expr::Call(expr) => {
                self.infer_call(expr, scope, expected_ty, expected_channel_type_arg)
            }
            Expr::Member(expr) => self.infer_member(expr, scope),
            Expr::Struct(expr) => {
                let expected_struct_ty = expected_ty
                    .filter(|expected| {
                        nominal_ty_name(expected)
                            .as_deref()
                            .is_some_and(|name| generic_base_assignable(name, &expr.name))
                    })
                    .cloned();
                let mut inferred_struct_substitutions = HashMap::new();
                if let Some(sig) = self.structs.get(&expr.name).cloned() {
                    let mut substitutions = expected_struct_ty
                        .as_ref()
                        .map(|expected| struct_substitutions(&sig.generics, expected))
                        .unwrap_or_default();
                    if expected_struct_ty.is_none() && !sig.generics.is_empty() {
                        for field in &expr.fields {
                            let Some(value) = &field.value else {
                                continue;
                            };
                            let Some(FieldSig {
                                ty: Ty::Named(generic),
                                ..
                            }) = sig.fields.get(&field.name)
                            else {
                                continue;
                            };
                            if !sig.generics.iter().any(|param| param == generic) {
                                continue;
                            }
                            let actual = self.infer_expr(value, scope);
                            if let Some(previous) = substitutions.get(generic) {
                                if previous != &actual {
                                    self.error(format!(
                                        "conflicting inferred type for `{generic}`: expected {}, got {}",
                                        display_ty(previous),
                                        display_ty(&actual)
                                    ));
                                }
                            } else {
                                substitutions.insert(generic.clone(), actual);
                            }
                        }
                    }
                    inferred_struct_substitutions = substitutions.clone();
                    for field in &expr.fields {
                        if let Some(field_sig) = sig.fields.get(&field.name) {
                            if !self.field_is_visible(&sig, field_sig) {
                                self.error(format!(
                                    "field `{}` on `{}` is private",
                                    field.name, expr.name
                                ));
                                if let Some(value) = &field.value {
                                    self.infer_expr(value, scope);
                                }
                                continue;
                            }
                            let expected = substitute_ty(&field_sig.ty, &substitutions);
                            if let Some(value) = &field.value {
                                if ty_mentions_generics(&expected, &sig.generics) {
                                    self.infer_expr(value, scope);
                                    continue;
                                }
                                let actual = self.infer_expr_with_expected(
                                    value,
                                    scope,
                                    Some(&expected),
                                    None,
                                );
                                self.expect_expr_assignable(
                                    &expected,
                                    &actual,
                                    value,
                                    "struct field type mismatch",
                                );
                            }
                        } else {
                            self.error(format!(
                                "unknown field `{}` on `{}`",
                                field.name, expr.name
                            ));
                        }
                    }
                }
                expected_struct_ty.unwrap_or_else(|| {
                    if let Some(sig) = self.structs.get(&expr.name) {
                        if !sig.generics.is_empty() {
                            return Ty::Generic {
                                base: expr.name.clone(),
                                args: sig
                                    .generics
                                    .iter()
                                    .map(|generic| {
                                        inferred_struct_substitutions
                                            .get(generic)
                                            .cloned()
                                            .unwrap_or(Ty::Unknown)
                                    })
                                    .collect(),
                            };
                        }
                    }
                    Ty::Named(expr.name.clone())
                })
            }
            Expr::Object(_) => Ty::Unknown,
            Expr::Closure(expr) => self.infer_closure(expr, scope),
            Expr::Match(expr) => self.infer_match(expr, scope),
            Expr::If(expr) => {
                let condition_ty = self.infer_expr(&expr.condition, scope);
                self.expect_assignable(&Ty::Bool, &condition_ty, "condition type mismatch");
                let expected_return = self.current_return_ty();
                let then_ty = self.check_child_block(&expr.then_branch, scope, &expected_return);
                if let Some(else_branch) = &expr.else_branch {
                    let else_ty = self.check_child_block(else_branch, scope, &expected_return);
                    join_branch_tys(then_ty, else_ty)
                } else {
                    Ty::Unit
                }
            }
            Expr::Block(block) => {
                let expected_return = self.current_return_ty();
                self.check_child_block(block, scope, &expected_return)
            }
        }
    }

    fn check_binary_operand_types(&mut self, op: BinaryOp, left: &Ty, right: &Ty) {
        match op {
            BinaryOp::BoolAnd | BinaryOp::BoolOr => {
                self.expect_assignable(&Ty::Bool, left, "boolean operator type mismatch");
                self.expect_assignable(&Ty::Bool, right, "boolean operator type mismatch");
            }
            BinaryOp::BitAnd
            | BinaryOp::BitOr
            | BinaryOp::BitXor
            | BinaryOp::Shl
            | BinaryOp::Shr => {
                if !is_int_like_ty(left) || !is_int_like_ty(right) {
                    self.error(format!(
                        "integer operator type mismatch: {} and {}",
                        display_ty(left),
                        display_ty(right)
                    ));
                }
            }
            BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem => {
                if !numeric_ty(left) || !numeric_ty(right) {
                    self.error(format!(
                        "numeric operator type mismatch: {} and {}",
                        display_ty(left),
                        display_ty(right)
                    ));
                }
            }
            BinaryOp::Add => {
                if is_string_like_ty(left) || is_string_like_ty(right) {
                    return;
                }
                if !numeric_ty(left) || !numeric_ty(right) {
                    self.error(format!(
                        "numeric operator type mismatch: {} and {}",
                        display_ty(left),
                        display_ty(right)
                    ));
                }
            }
            _ => {}
        }
    }

    fn current_return_ty(&self) -> Ty {
        self.return_tys.last().cloned().unwrap_or(Ty::Unknown)
    }

    fn infer_try_expr(&mut self, expr: &Expr, scope: &mut HashMap<String, Ty>) -> Ty {
        let inner_ty = self.infer_expr(expr, scope);
        match &inner_ty {
            Ty::Generic { base, args } if generic_base_assignable(base, "Result") => {
                let ok_ty = args.first().cloned().unwrap_or(Ty::Unknown);
                let err_ty = args.get(1).cloned().unwrap_or(Ty::Unknown);
                match self.return_tys.last().cloned() {
                    Some(Ty::Generic {
                        base: return_base,
                        args: return_args,
                    }) if generic_base_assignable(&return_base, "Result") => {
                        if let Some(return_err) = return_args.get(1) {
                            self.expect_assignable(return_err, &err_ty, "`?` error type mismatch");
                        }
                    }
                    Some(return_ty) if !matches!(return_ty, Ty::Unknown) => {
                        self.error(format!(
                            "`?` on Result requires enclosing return type Result<_, _>, got {}",
                            display_ty(&return_ty)
                        ));
                    }
                    _ => {}
                }
                ok_ty
            }
            Ty::Generic { base, args } if generic_base_assignable(base, "Option") => {
                let some_ty = args.first().cloned().unwrap_or(Ty::Unknown);
                match self.return_tys.last().cloned() {
                    Some(Ty::Generic {
                        base: return_base, ..
                    }) if generic_base_assignable(&return_base, "Option") => {}
                    Some(return_ty) if !matches!(return_ty, Ty::Unknown) => {
                        self.error(format!(
                            "`?` on Option requires enclosing return type Option<_>, got {}",
                            display_ty(&return_ty)
                        ));
                    }
                    _ => {}
                }
                some_ty
            }
            Ty::Unknown => Ty::Unknown,
            other => {
                self.error(format!(
                    "`?` requires Result<T, E> or Option<T>, got {}",
                    display_ty(other)
                ));
                Ty::Unknown
            }
        }
    }

    fn method_function_sig(&self, receiver_ty: &Ty, method: &str) -> Option<FunctionSig> {
        if method == "len" && is_string_like_ty(receiver_ty) {
            return Some(FunctionSig {
                generics: Vec::new(),
                params: vec![Ty::Str],
                ret: Ty::Int(IntKind::Int),
            });
        }
        if method == "to_string" && matches!(receiver_ty, Ty::Int(_)) {
            return Some(FunctionSig {
                generics: Vec::new(),
                params: vec![Ty::Int(IntKind::Int)],
                ret: Ty::String,
            });
        }

        let receiver_name = nominal_ty_name(receiver_ty);
        if let Some(sig) = receiver_name
            .as_ref()
            .and_then(|name| self.methods.get(name))
            .and_then(|methods| methods.get(method))
            .cloned()
        {
            return Some(sig);
        }
        if let Some(sig) = receiver_name
            .as_ref()
            .and_then(|name| self.interfaces.get(name))
            .and_then(|interface| interface.methods.get(method))
            .cloned()
        {
            return Some(sig);
        }

        let suffix = format!("__{method}");
        self.functions
            .iter()
            .filter(|(name, _)| name.as_str() == method || name.ends_with(&suffix))
            .find_map(|(_, sig)| {
                sig.params
                    .first()
                    .filter(|expected| receiver_assignable(expected, receiver_ty))
                    .map(|_| sig.clone())
            })
    }

    fn infer_call(
        &mut self,
        expr: &CallExpr,
        scope: &mut HashMap<String, Ty>,
        expected_ty: Option<&Ty>,
        expected_channel_type_arg: Option<&str>,
    ) -> Ty {
        if let Expr::Ident(name) = expr.callee.as_ref() {
            if let Some(sig) = self.enum_variants.get(name).cloned() {
                let substitutions =
                    expected_ty.and_then(|expected| enum_constructor_substitutions(&sig, expected));
                let mut actuals = Vec::new();
                if sig.params.len() != expr.args.len() {
                    self.error(format!(
                        "enum variant `{name}` expects {} argument(s), got {}",
                        sig.params.len(),
                        expr.args.len()
                    ));
                }
                for (index, (expected, arg)) in sig.params.iter().zip(&expr.args).enumerate() {
                    let expected = substitutions
                        .as_ref()
                        .map(|substitutions| substitute_ty(expected, substitutions))
                        .unwrap_or_else(|| expected.clone());
                    let actual = self.infer_expr_with_expected(arg, scope, Some(&expected), None);
                    actuals.push(actual.clone());
                    if ty_mentions_generics(&expected, &sig.enum_generics) {
                        continue;
                    }
                    self.expect_expr_assignable(
                        &expected,
                        &actual,
                        arg,
                        &format!(
                            "argument {} type mismatch in enum variant `{name}`",
                            index + 1
                        ),
                    );
                }
                if let Some(expected) = expected_ty.filter(|expected| {
                    nominal_ty_name(expected)
                        .as_deref()
                        .is_some_and(|name| generic_base_assignable(name, &sig.enum_name))
                }) {
                    return expected.clone();
                }
                if !sig.enum_generics.is_empty() {
                    return Ty::Generic {
                        base: sig.enum_name.clone(),
                        args: infer_enum_constructor_args(&sig, &actuals),
                    };
                }
                return Ty::Named(sig.enum_name);
            }

            let inferred_type_arg = if expr.type_args.is_none() {
                infer_channel_call_type_arg(name, expr, scope, self, expected_channel_type_arg)
            } else {
                None
            };
            if let Some(specialized) = specialize_channel_callee(
                name,
                expr.type_args.as_deref().or(inferred_type_arg.as_deref()),
            ) {
                if let Some(sig) = self.functions.get(&specialized).cloned() {
                    if sig.params.len() != expr.args.len() {
                        self.error(format!(
                            "`{name}` expects {} argument(s), got {}",
                            sig.params.len(),
                            expr.args.len()
                        ));
                    }
                    for (index, (expected, arg)) in sig.params.iter().zip(&expr.args).enumerate() {
                        let actual =
                            self.infer_expr_with_expected(arg, scope, Some(expected), None);
                        self.expect_expr_assignable(
                            expected,
                            &actual,
                            arg,
                            &format!("argument {} type mismatch in call to `{name}`", index + 1),
                        );
                    }
                    return sig.ret;
                }
            }
        }
        if matches!(expr.callee.as_ref(), Expr::Ident(name) if name == "raw_write") {
            for arg in &expr.args {
                self.infer_expr(arg, scope);
            }
            return Ty::Int(IntKind::Int);
        }
        if matches!(expr.callee.as_ref(), Expr::Ident(name) if name == "raw_alloc") {
            for arg in &expr.args {
                self.infer_expr(arg, scope);
            }
            return Ty::Int(IntKind::Int);
        }
        if matches!(expr.callee.as_ref(), Expr::Ident(name) if name == "raw_alloc_string") {
            for arg in &expr.args {
                self.infer_expr(arg, scope);
            }
            return Ty::String;
        }
        if matches!(expr.callee.as_ref(), Expr::Ident(name) if name == "raw_load8" || name == "raw_load64" || name == "raw_string_ptr" || name == "raw_string_clone_ptr" || name == "raw_function_ptr" || name == "raw_float_to_int")
        {
            for arg in &expr.args {
                self.infer_expr(arg, scope);
            }
            return Ty::Int(IntKind::Int);
        }
        if matches!(expr.callee.as_ref(), Expr::Ident(name) if name == "raw_set_nonblocking") {
            for arg in &expr.args {
                self.infer_expr(arg, scope);
            }
            return Ty::Int(IntKind::Int);
        }
        if matches!(expr.callee.as_ref(), Expr::Ident(name) if name == "raw_thread_spawn" || name == "raw_thread_join")
        {
            for arg in &expr.args {
                self.infer_expr(arg, scope);
            }
            return Ty::Int(IntKind::Int);
        }
        if matches!(expr.callee.as_ref(), Expr::Ident(name) if name == "raw_mem_alloc_count" || name == "raw_mem_free_count" || name == "raw_mem_live_bytes" || name == "raw_mem_high_water_bytes")
        {
            for arg in &expr.args {
                self.infer_expr(arg, scope);
            }
            return Ty::Int(IntKind::Int);
        }
        if matches!(expr.callee.as_ref(), Expr::Ident(name) if name == "raw_store8" || name == "raw_store64" || name == "raw_string_store8" || name == "raw_free")
        {
            for arg in &expr.args {
                self.infer_expr(arg, scope);
            }
            return Ty::Unit;
        }
        if matches!(expr.callee.as_ref(), Expr::Ident(name) if name == "raw_strlen") {
            for arg in &expr.args {
                self.infer_expr(arg, scope);
            }
            return Ty::Int(IntKind::Int);
        }
        if matches!(expr.callee.as_ref(), Expr::Ident(name) if name == "raw_string_concat" || name == "raw_int_to_string" || name == "raw_string_from_ptr")
        {
            for arg in &expr.args {
                self.infer_expr(arg, scope);
            }
            return Ty::String;
        }
        if matches!(expr.callee.as_ref(), Expr::Ident(name) if name == "raw_function_from_ptr") {
            for arg in &expr.args {
                self.infer_expr(arg, scope);
            }
            return Ty::Fn {
                params: Vec::new(),
                ret: Box::new(Ty::Unit),
                is_async: false,
            };
        }
        if matches!(expr.callee.as_ref(), Expr::Ident(name) if name == "raw_function_from_ptr_int")
        {
            for arg in &expr.args {
                self.infer_expr(arg, scope);
            }
            return Ty::Fn {
                params: vec![Ty::Int(IntKind::Int)],
                ret: Box::new(Ty::Unit),
                is_async: false,
            };
        }
        if matches!(expr.callee.as_ref(), Expr::Ident(name) if name == "raw_function_from_ptr_handler")
        {
            for arg in &expr.args {
                self.infer_expr(arg, scope);
            }
            return Ty::Fn {
                params: vec![Ty::Int(IntKind::Int), Ty::Str],
                ret: Box::new(Ty::Int(IntKind::Int)),
                is_async: false,
            };
        }
        if matches!(expr.callee.as_ref(), Expr::Ident(name) if name == "raw_function_from_ptr_request_handler")
        {
            for arg in &expr.args {
                self.infer_expr(arg, scope);
            }
            return Ty::Fn {
                params: vec![
                    Ty::Int(IntKind::Int),
                    Ty::Named("http__Request".to_string()),
                    Ty::Str,
                ],
                ret: Box::new(Ty::Int(IntKind::Int)),
                is_async: false,
            };
        }
        if matches!(expr.callee.as_ref(), Expr::Ident(name) if name == "raw_function_from_ptr_response_handler")
        {
            for arg in &expr.args {
                self.infer_expr(arg, scope);
            }
            return Ty::Fn {
                params: vec![
                    Ty::Int(IntKind::Int),
                    Ty::Named("http__Request".to_string()),
                    Ty::Str,
                ],
                ret: Box::new(Ty::Named("http__Response".to_string())),
                is_async: false,
            };
        }
        let member_function_field_ty = match expr.callee.as_ref() {
            Expr::Member(member) => self
                .member_field_ty(member, scope)
                .filter(|ty| matches!(ty, Ty::Fn { .. })),
            _ => None,
        };
        let function_value_ty = match expr.callee.as_ref() {
            Expr::Ident(name) if matches!(scope.get(name), Some(Ty::Fn { .. })) => {
                scope.get(name).cloned()
            }
            Expr::Member(_) => member_function_field_ty.clone(),
            _ => None,
        };
        if let Some(Ty::Fn {
            params,
            ret,
            is_async: false,
        }) = function_value_ty
        {
            if params.len() != expr.args.len() {
                self.error(format!(
                    "function value expects {} argument(s), got {}",
                    params.len(),
                    expr.args.len()
                ));
            }
            for (index, (expected, arg)) in params.iter().zip(&expr.args).enumerate() {
                let actual = self.infer_expr_with_expected(arg, scope, Some(expected), None);
                self.expect_expr_assignable(
                    expected,
                    &actual,
                    arg,
                    &format!(
                        "argument {} type mismatch in function value call",
                        index + 1
                    ),
                );
            }
            return *ret;
        }
        if let Expr::Member(member) = expr.callee.as_ref() {
            let receiver_ty = self.infer_expr(&member.target, scope);
            if !matches!(receiver_ty, Ty::Unknown) {
                if let Some(sig) = self.method_function_sig(&receiver_ty, &member.member) {
                    if sig.params.len() != expr.args.len() + 1 {
                        self.error(format!(
                            "method `{}` expects {} argument(s), got {}",
                            member.member,
                            sig.params.len() - 1,
                            expr.args.len()
                        ));
                    }
                    if let Some(expected) = sig.params.first() {
                        self.expect_expr_assignable(
                            expected,
                            &receiver_ty,
                            &member.target,
                            &format!("receiver type mismatch in call to `{}`", member.member),
                        );
                    }
                    for (index, (expected, arg)) in
                        sig.params.iter().skip(1).zip(&expr.args).enumerate()
                    {
                        let actual =
                            self.infer_expr_with_expected(arg, scope, Some(expected), None);
                        self.expect_expr_assignable(
                            expected,
                            &actual,
                            arg,
                            &format!(
                                "argument {} type mismatch in call to `{}`",
                                index + 1,
                                member.member
                            ),
                        );
                    }
                    return sig.ret;
                } else if self.is_known_receiver_type(&receiver_ty) {
                    self.error(format!(
                        "unknown method `{}` for `{}`",
                        member.member,
                        display_ty(&receiver_ty)
                    ));
                }
            }
        }
        let function_value_ty = match expr.callee.as_ref() {
            Expr::Ident(name) if matches!(scope.get(name), Some(Ty::Fn { .. })) => {
                scope.get(name).cloned()
            }
            Expr::Member(_) => member_function_field_ty,
            _ => None,
        };
        if let Some(Ty::Fn {
            params,
            ret,
            is_async: false,
        }) = function_value_ty
        {
            if params.len() != expr.args.len() {
                self.error(format!(
                    "function value expects {} argument(s), got {}",
                    params.len(),
                    expr.args.len()
                ));
            }
            for (index, (expected, arg)) in params.iter().zip(&expr.args).enumerate() {
                let actual = self.infer_expr_with_expected(arg, scope, Some(expected), None);
                self.expect_expr_assignable(
                    expected,
                    &actual,
                    arg,
                    &format!(
                        "argument {} type mismatch in function value call",
                        index + 1
                    ),
                );
            }
            return *ret;
        }
        if let Expr::Ident(name) = expr.callee.as_ref() {
            if let Some(sig) = self.functions.get(name).cloned() {
                let sig = self.function_sig_for_call(name, sig, expr, scope, expected_ty);
                if sig.params.len() != expr.args.len() {
                    self.error(format!(
                        "`{name}` expects {} argument(s), got {}",
                        sig.params.len(),
                        expr.args.len()
                    ));
                }
                for (index, (expected, arg)) in sig.params.iter().zip(&expr.args).enumerate() {
                    let actual = self.infer_expr_with_expected(arg, scope, Some(expected), None);
                    self.expect_expr_assignable(
                        expected,
                        &actual,
                        arg,
                        &format!("argument {} type mismatch in call to `{name}`", index + 1),
                    );
                }
                return sig.ret;
            }
        }
        self.infer_expr(&expr.callee, scope);
        for arg in &expr.args {
            self.infer_expr(arg, scope);
        }
        Ty::Unknown
    }

    fn is_known_receiver_type(&self, receiver_ty: &Ty) -> bool {
        if matches!(receiver_ty, Ty::Int(_) | Ty::Str | Ty::String) {
            return true;
        }
        let Some(name) = nominal_ty_name(receiver_ty) else {
            return false;
        };
        self.structs.contains_key(&name)
            || self.enums.contains_key(&name)
            || self.interfaces.contains_key(&name)
            || self.methods.contains_key(&name)
    }

    fn function_sig_for_call(
        &mut self,
        name: &str,
        sig: FunctionSig,
        expr: &CallExpr,
        scope: &mut HashMap<String, Ty>,
        expected_ty: Option<&Ty>,
    ) -> FunctionSig {
        if expr.type_args.is_some() {
            self.specialize_function_sig(name, sig, expr.type_args.as_deref())
        } else {
            self.infer_function_sig(name, sig, expr, scope, expected_ty)
        }
    }

    fn specialize_function_sig(
        &mut self,
        name: &str,
        sig: FunctionSig,
        type_args: Option<&str>,
    ) -> FunctionSig {
        let Some(type_args) = type_args else {
            return sig;
        };
        if sig.generics.is_empty() {
            self.error(format!("`{name}` is not generic"));
            return sig;
        }
        let args = parse_type_arg_tys(type_args);
        if args.len() != sig.generics.len() {
            self.error(format!(
                "`{name}` expects {} type argument(s), got {}",
                sig.generics.len(),
                args.len()
            ));
            return sig;
        }
        let substitutions: HashMap<_, _> = sig.generics.iter().cloned().zip(args).collect();
        FunctionSig {
            generics: Vec::new(),
            params: sig
                .params
                .iter()
                .map(|param| substitute_ty(param, &substitutions))
                .collect(),
            ret: substitute_ty(&sig.ret, &substitutions),
        }
    }

    fn infer_function_sig(
        &mut self,
        name: &str,
        sig: FunctionSig,
        expr: &CallExpr,
        scope: &mut HashMap<String, Ty>,
        expected_ty: Option<&Ty>,
    ) -> FunctionSig {
        if sig.generics.is_empty() {
            return sig;
        }
        let mut substitutions = HashMap::new();
        if let Some(expected_ty) = expected_ty {
            self.collect_generic_substitutions(
                name,
                &sig.ret,
                expected_ty,
                &sig.generics,
                &mut substitutions,
            );
        }
        for (param, arg) in sig.params.iter().zip(&expr.args) {
            let actual = self.infer_expr(arg, scope);
            self.collect_generic_substitutions(
                name,
                param,
                &default_local_ty(actual),
                &sig.generics,
                &mut substitutions,
            );
        }
        FunctionSig {
            generics: Vec::new(),
            params: sig
                .params
                .iter()
                .map(|param| substitute_ty(param, &substitutions))
                .collect(),
            ret: substitute_ty(&sig.ret, &substitutions),
        }
    }

    fn collect_generic_substitutions(
        &mut self,
        function_name: &str,
        pattern: &Ty,
        actual: &Ty,
        generics: &[String],
        substitutions: &mut HashMap<String, Ty>,
    ) {
        match pattern {
            Ty::Named(name) if generics.iter().any(|generic| generic == name) => {
                if matches!(actual, Ty::Unknown) {
                    return;
                }
                if let Some(existing) = substitutions.get(name) {
                    if existing != actual {
                        self.error(format!(
                            "conflicting inferred type for `{name}` in call to `{function_name}`: expected {}, got {}",
                            display_ty(existing),
                            display_ty(actual)
                        ));
                    }
                } else {
                    substitutions.insert(name.clone(), actual.clone());
                }
            }
            Ty::Generic {
                base: pattern_base,
                args: pattern_args,
            } => {
                if let Ty::Generic {
                    base: actual_base,
                    args: actual_args,
                } = actual
                {
                    if generic_base_assignable(pattern_base, actual_base) {
                        for (pattern_arg, actual_arg) in pattern_args.iter().zip(actual_args) {
                            self.collect_generic_substitutions(
                                function_name,
                                pattern_arg,
                                actual_arg,
                                generics,
                                substitutions,
                            );
                        }
                    }
                }
            }
            Ty::Fn {
                params: pattern_params,
                ret: pattern_ret,
                ..
            } => {
                if let Ty::Fn {
                    params: actual_params,
                    ret: actual_ret,
                    ..
                } = actual
                {
                    for (pattern_param, actual_param) in pattern_params.iter().zip(actual_params) {
                        self.collect_generic_substitutions(
                            function_name,
                            pattern_param,
                            actual_param,
                            generics,
                            substitutions,
                        );
                    }
                    self.collect_generic_substitutions(
                        function_name,
                        pattern_ret,
                        actual_ret,
                        generics,
                        substitutions,
                    );
                }
            }
            _ => {}
        }
    }

    fn infer_closure(&mut self, expr: &ClosureExpr, scope: &mut HashMap<String, Ty>) -> Ty {
        let params = expr
            .params
            .iter()
            .map(|param| {
                param
                    .ty_expr
                    .as_ref()
                    .map(type_from_expr)
                    .unwrap_or(Ty::Unknown)
            })
            .collect::<Vec<_>>();
        let ret = expr
            .return_type_expr
            .as_ref()
            .map(type_from_expr)
            .unwrap_or(Ty::Unit);
        let mut child = scope.clone();
        for (param, ty) in expr.params.iter().zip(params.iter()) {
            child.insert(normalize_param_name(&param.name), ty.clone());
        }
        self.return_tys.push(ret.clone());
        self.check_block(&expr.body, &mut child, &ret);
        self.return_tys.pop();
        Ty::Fn {
            params,
            ret: Box::new(ret),
            is_async: expr.is_async,
        }
    }

    fn infer_member(&mut self, expr: &MemberExpr, scope: &mut HashMap<String, Ty>) -> Ty {
        let target_ty = self.infer_expr(&expr.target, scope);
        if let Some(name) = nominal_ty_name(&target_ty) {
            if let Some(sig) = self.structs.get(&name) {
                if let Some(field_sig) = sig.fields.get(&expr.member) {
                    if !self.field_is_visible(sig, field_sig) {
                        self.error(format!("field `{}` on `{name}` is private", expr.member));
                        return Ty::Unknown;
                    }
                    let substitutions = struct_substitutions(&sig.generics, &target_ty);
                    return substitute_ty(&field_sig.ty, &substitutions);
                }
                self.error(format!("unknown field `{}` on `{name}`", expr.member));
            }
        }
        Ty::Unknown
    }

    fn member_field_ty(
        &mut self,
        expr: &MemberExpr,
        scope: &mut HashMap<String, Ty>,
    ) -> Option<Ty> {
        let target_ty = self.infer_expr(&expr.target, scope);
        let name = nominal_ty_name(&target_ty)?;
        let sig = self.structs.get(&name)?;
        let field_sig = sig.fields.get(&expr.member)?;
        if !self.field_is_visible(sig, field_sig) {
            self.error(format!("field `{}` on `{name}` is private", expr.member));
            return None;
        }
        let substitutions = struct_substitutions(&sig.generics, &target_ty);
        Some(substitute_ty(&field_sig.ty, &substitutions))
    }

    fn field_is_visible(&self, sig: &StructSig, field: &FieldSig) -> bool {
        sig.module == self.current_module || field.public
    }

    fn infer_match(&mut self, expr: &MatchExpr, scope: &mut HashMap<String, Ty>) -> Ty {
        let value_ty = self.infer_expr(&expr.value, scope);
        self.check_match_patterns(&value_ty, expr);
        let mut result = None;
        for arm in &expr.arms {
            let mut child = scope.clone();
            let pattern_tys = self.pattern_binding_tys(&value_ty, &arm.pattern);
            for binding in pattern_bindings(&arm.pattern) {
                let ty = pattern_tys.get(&binding).cloned().unwrap_or(Ty::Unknown);
                child.insert(binding, ty);
            }
            let arm_ty = self.infer_expr(&arm.body, &mut child);
            result = Some(match result {
                Some(result) => join_branch_tys(result, arm_ty),
                None => arm_ty,
            });
        }
        result.unwrap_or(Ty::Unknown)
    }

    fn check_match_patterns(&mut self, value_ty: &Ty, expr: &MatchExpr) {
        let Some(enum_name) = nominal_ty_name(value_ty) else {
            return;
        };
        let Some(variants) = self.enums.get(&enum_name).cloned() else {
            return;
        };

        let mut covered = HashSet::new();
        let mut has_catchall = false;

        for arm in &expr.arms {
            let pattern = arm.pattern.trim();
            if pattern == "_" || starts_with_lowercase_binding(pattern) {
                has_catchall = true;
                continue;
            }

            if let Some(variant) = pattern_variant(pattern) {
                if let Some(sig) = variants.variants.get(&variant) {
                    let expected = sig.params.len();
                    let actual = pattern_payload_count(pattern);
                    if expected != actual {
                        self.error(format!(
                            "match pattern `{variant}` expects {expected} binding(s), got {actual}"
                        ));
                    }
                    covered.insert(variant);
                } else {
                    self.error(format!(
                        "unknown variant `{variant}` for enum `{enum_name}`"
                    ));
                }
            }
        }

        if !has_catchall {
            let missing: Vec<_> = variants
                .variants
                .keys()
                .filter(|variant| !covered.contains(*variant))
                .cloned()
                .collect();
            if !missing.is_empty() {
                self.error(format!(
                    "non-exhaustive match on `{enum_name}`; missing {}",
                    missing.join(", ")
                ));
            }
        }
    }

    fn pattern_binding_tys(&self, value_ty: &Ty, pattern: &str) -> HashMap<String, Ty> {
        let Some(enum_name) = nominal_ty_name(value_ty) else {
            return HashMap::new();
        };
        let Some(variant_name) = pattern_variant(pattern) else {
            return HashMap::new();
        };
        let Some(enum_sig) = self.enums.get(&enum_name) else {
            return HashMap::new();
        };
        let Some(variant) = enum_sig.variants.get(&variant_name) else {
            return HashMap::new();
        };
        let substitutions = enum_substitutions(&enum_sig.generics, value_ty);
        pattern_bindings(pattern)
            .into_iter()
            .zip(variant.params.iter())
            .map(|(binding, ty)| (binding, substitute_ty(ty, &substitutions)))
            .collect()
    }

    fn expect_assignable(&mut self, expected: &Ty, actual: &Ty, label: &str) {
        if matches!(expected, Ty::Unknown) || matches!(actual, Ty::Unknown | Ty::Never) {
            return;
        }
        if int_assignable(expected, actual) {
            return;
        }
        if raw_ptr_bridge_assignable(expected, actual) {
            return;
        }
        if string_view_assignable(expected, actual) {
            return;
        }
        if generic_assignable(expected, actual) {
            return;
        }
        if self.interface_assignable(expected, actual) {
            return;
        }
        if expected != actual {
            self.error(format!(
                "{label}: expected {}, got {}",
                display_ty(expected),
                display_ty(actual)
            ));
        }
    }

    fn expect_expr_assignable(&mut self, expected: &Ty, actual: &Ty, expr: &Expr, label: &str) {
        if matches!(expected, Ty::Unknown) || matches!(actual, Ty::Unknown | Ty::Never) {
            return;
        }
        if let (Ty::Int(expected), Ty::Int(actual)) = (expected, actual) {
            if *actual == IntKind::Untyped {
                if let Some(value) = int_literal_value(expr) {
                    if int_literal_fits(*expected, value) {
                        return;
                    }
                    self.error(format!(
                        "{label}: integer literal {value} does not fit {}",
                        display_ty(&Ty::Int(*expected))
                    ));
                    return;
                }
            }
        }
        self.expect_assignable(expected, actual, label);
    }

    fn interface_assignable(&self, expected: &Ty, actual: &Ty) -> bool {
        let Ty::Named(interface_name) = expected else {
            return false;
        };
        let Some(interface) = self.interfaces.get(interface_name) else {
            return false;
        };
        if interface_name == "Display" && is_string_like_ty(actual) {
            return true;
        }
        let Some(actual_name) = nominal_ty_name(actual) else {
            return false;
        };
        let Some(methods) = self.methods.get(&actual_name) else {
            return false;
        };
        interface
            .methods
            .keys()
            .all(|method| methods.contains_key(method))
    }

    fn error(&mut self, message: String) {
        self.diagnostics.push(Diagnostic {
            message,
            location: None,
            code: None,
        });
    }

    fn finish(self) -> Result<TypeReport, Vec<Diagnostic>> {
        if self.diagnostics.is_empty() {
            Ok(TypeReport {
                checked_functions: self.checked_functions,
            })
        } else {
            Err(self.diagnostics)
        }
    }
}

fn type_from_expr(expr: &TypeExpr) -> Ty {
    match expr {
        TypeExpr::Path(path) => path
            .first()
            .map(|name| match name.as_str() {
                "Bool" => Ty::Bool,
                "Int" => Ty::Int(IntKind::Int),
                "Int8" => Ty::Int(IntKind::Int8),
                "Int16" => Ty::Int(IntKind::Int16),
                "Int32" => Ty::Int(IntKind::Int32),
                "Int64" => Ty::Int(IntKind::Int64),
                "UInt" => Ty::Int(IntKind::UInt),
                "UInt8" => Ty::Int(IntKind::UInt8),
                "UInt16" => Ty::Int(IntKind::UInt16),
                "UInt32" => Ty::Int(IntKind::UInt32),
                "UInt64" => Ty::Int(IntKind::UInt64),
                "Float32" | "Float64" => Ty::Float,
                "Str" => Ty::Str,
                "String" => Ty::String,
                "Never" => Ty::Never,
                other => Ty::Named(other.to_string()),
            })
            .unwrap_or(Ty::Unknown),
        TypeExpr::Tuple(items) if items.is_empty() => Ty::Unit,
        TypeExpr::Tuple(_) => Ty::Unknown,
        TypeExpr::Generic { base, args } => Ty::Generic {
            base: type_base_name(base).unwrap_or_else(|| display_type_expr(base)),
            args: args.iter().map(type_from_expr).collect(),
        },
        TypeExpr::Ref { inner, .. } | TypeExpr::Impl(inner) | TypeExpr::Mut(inner) => {
            type_from_expr(inner)
        }
        TypeExpr::RawPtr { mutable, .. } => Ty::RawPtr { mutable: *mutable },
        TypeExpr::Fn {
            is_async,
            params,
            return_type,
        } => Ty::Fn {
            params: params.iter().map(type_from_expr).collect(),
            ret: Box::new(type_from_expr(return_type)),
            is_async: *is_async,
        },
        TypeExpr::Missing => Ty::Unknown,
    }
}

fn function_item_sig(function: &FunctionItem) -> FunctionSig {
    FunctionSig {
        generics: parse_generic_params(function.generics.as_deref()),
        params: function
            .params
            .iter()
            .map(|param| {
                param
                    .ty_expr
                    .as_ref()
                    .map(type_from_expr)
                    .unwrap_or(Ty::Unknown)
            })
            .collect(),
        ret: function
            .return_type_expr
            .as_ref()
            .map(type_from_expr)
            .unwrap_or(Ty::Unit),
    }
}

fn fn_ty(sig: &FunctionSig) -> Ty {
    Ty::Fn {
        params: sig.params.clone(),
        ret: Box::new(sig.ret.clone()),
        is_async: false,
    }
}

fn join_branch_tys(left: Ty, right: Ty) -> Ty {
    match (left, right) {
        (Ty::Never, ty) | (ty, Ty::Never) => ty,
        (left, right) if left == right => left,
        _ => Ty::Unknown,
    }
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

fn variant_payload_tys(payload: Option<&str>, enum_generics: &[String]) -> Vec<Ty> {
    let Some(payload) = payload.map(str::trim).filter(|value| !value.is_empty()) else {
        return Vec::new();
    };
    if payload.starts_with('{') {
        return Vec::new();
    }
    split_top_level_commas(payload)
        .into_iter()
        .map(|part| ty_from_payload_text(&part, enum_generics))
        .collect()
}

fn parse_type_arg_tys(type_args: &str) -> Vec<Ty> {
    split_top_level_commas(type_args)
        .into_iter()
        .map(|part| ty_from_payload_text(&part, &[]))
        .collect()
}

fn split_top_level_commas(value: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;
    for (index, ch) in value.char_indices() {
        match ch {
            '<' | '(' | '{' => depth += 1,
            '>' | ')' | '}' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                parts.push(value[start..index].trim().to_string());
                start = index + ch.len_utf8();
            }
            _ => {}
        }
    }
    parts.push(value[start..].trim().to_string());
    parts.into_iter().filter(|part| !part.is_empty()).collect()
}

fn ty_from_payload_text(value: &str, enum_generics: &[String]) -> Ty {
    let value = value.trim();
    if let Some(inner) = value.strip_prefix('&') {
        return ty_from_payload_text(inner.trim_start_matches("mut").trim(), enum_generics);
    }
    if let Some(function) = fn_ty_from_payload_text(value, enum_generics) {
        return function;
    }
    if let Some((base, args)) = split_generic_payload(value) {
        return Ty::Generic {
            base: normalize_payload_name(base),
            args: split_top_level_commas(args)
                .into_iter()
                .map(|arg| ty_from_payload_text(&arg, enum_generics))
                .collect(),
        };
    }
    match value {
        "Bool" => Ty::Bool,
        "Int" => Ty::Int(IntKind::Int),
        "Int8" => Ty::Int(IntKind::Int8),
        "Int16" => Ty::Int(IntKind::Int16),
        "Int32" => Ty::Int(IntKind::Int32),
        "Int64" => Ty::Int(IntKind::Int64),
        "UInt" => Ty::Int(IntKind::UInt),
        "UInt8" => Ty::Int(IntKind::UInt8),
        "UInt16" => Ty::Int(IntKind::UInt16),
        "UInt32" => Ty::Int(IntKind::UInt32),
        "UInt64" => Ty::Int(IntKind::UInt64),
        "Float32" | "Float64" => Ty::Float,
        "Str" => Ty::Str,
        "String" => Ty::String,
        other => Ty::Named(normalize_payload_name(other)),
    }
}

fn fn_ty_from_payload_text(value: &str, enum_generics: &[String]) -> Option<Ty> {
    let rest = value
        .strip_prefix("Fn")
        .or_else(|| value.strip_prefix("fn"))?
        .trim();
    let params_start = rest.find('(')?;
    let params_end = matching_paren_index(rest, params_start)?;
    let params_text = &rest[params_start + 1..params_end];
    let return_text = rest[params_end + 1..].trim().strip_prefix("->")?.trim();
    Some(Ty::Fn {
        params: split_top_level_commas(params_text)
            .into_iter()
            .map(|part| ty_from_payload_text(&part, enum_generics))
            .collect(),
        ret: Box::new(ty_from_payload_text(return_text, enum_generics)),
        is_async: false,
    })
}

fn matching_paren_index(value: &str, open_index: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (index, ch) in value
        .char_indices()
        .skip_while(|(index, _)| *index < open_index)
    {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

fn normalize_payload_name(value: &str) -> String {
    value.trim().replace('.', "__")
}

fn split_generic_payload(value: &str) -> Option<(&str, &str)> {
    let open = value.find('<')?;
    if !value.ends_with('>') {
        return None;
    }
    Some((value[..open].trim(), &value[open + 1..value.len() - 1]))
}

fn enum_substitutions(generics: &[String], value_ty: &Ty) -> HashMap<String, Ty> {
    let Ty::Generic { args, .. } = value_ty else {
        return HashMap::new();
    };
    generics.iter().cloned().zip(args.iter().cloned()).collect()
}

fn struct_substitutions(generics: &[String], value_ty: &Ty) -> HashMap<String, Ty> {
    let Ty::Generic { args, .. } = value_ty else {
        return HashMap::new();
    };
    generics.iter().cloned().zip(args.iter().cloned()).collect()
}

fn enum_constructor_substitutions(
    sig: &VariantSig,
    expected_ty: &Ty,
) -> Option<HashMap<String, Ty>> {
    let Ty::Generic { base, args } = expected_ty else {
        return None;
    };
    if !generic_base_assignable(base, &sig.enum_name) || args.len() != sig.enum_generics.len() {
        return None;
    }
    Some(
        sig.enum_generics
            .iter()
            .cloned()
            .zip(args.iter().cloned())
            .collect(),
    )
}

fn infer_enum_constructor_args(sig: &VariantSig, actuals: &[Ty]) -> Vec<Ty> {
    let mut substitutions = HashMap::new();
    for (expected, actual) in sig.params.iter().zip(actuals) {
        collect_direct_generic_substitution(
            expected,
            actual,
            &sig.enum_generics,
            &mut substitutions,
        );
    }
    sig.enum_generics
        .iter()
        .map(|generic| substitutions.get(generic).cloned().unwrap_or(Ty::Unknown))
        .collect()
}

fn collect_direct_generic_substitution(
    expected: &Ty,
    actual: &Ty,
    generics: &[String],
    substitutions: &mut HashMap<String, Ty>,
) {
    match expected {
        Ty::Named(name) if generics.iter().any(|generic| generic == name) => {
            substitutions
                .entry(name.clone())
                .or_insert_with(|| default_local_ty(actual.clone()));
        }
        Ty::Generic {
            base: expected_base,
            args: expected_args,
        } => {
            let Ty::Generic {
                base: actual_base,
                args: actual_args,
            } = actual
            else {
                return;
            };
            if !generic_base_assignable(expected_base, actual_base) {
                return;
            }
            for (expected_arg, actual_arg) in expected_args.iter().zip(actual_args) {
                collect_direct_generic_substitution(
                    expected_arg,
                    actual_arg,
                    generics,
                    substitutions,
                );
            }
        }
        _ => {}
    }
}

fn substitute_ty(ty: &Ty, substitutions: &HashMap<String, Ty>) -> Ty {
    match ty {
        Ty::Named(name) => substitutions
            .get(name)
            .cloned()
            .unwrap_or_else(|| ty.clone()),
        Ty::Generic { base, args } => Ty::Generic {
            base: base.clone(),
            args: args
                .iter()
                .map(|arg| substitute_ty(arg, substitutions))
                .collect(),
        },
        Ty::Fn {
            params,
            ret,
            is_async,
        } => Ty::Fn {
            params: params
                .iter()
                .map(|param| substitute_ty(param, substitutions))
                .collect(),
            ret: Box::new(substitute_ty(ret, substitutions)),
            is_async: *is_async,
        },
        _ => ty.clone(),
    }
}

fn ty_mentions_generics(ty: &Ty, generics: &[String]) -> bool {
    match ty {
        Ty::Named(name) => generics.iter().any(|generic| generic == name),
        Ty::Generic { args, .. } => args.iter().any(|arg| ty_mentions_generics(arg, generics)),
        Ty::Fn { params, ret, .. } => {
            params
                .iter()
                .any(|param| ty_mentions_generics(param, generics))
                || ty_mentions_generics(ret, generics)
        }
        _ => false,
    }
}

fn display_ty(ty: &Ty) -> String {
    match ty {
        Ty::Unknown => "Unknown".to_string(),
        Ty::Never => "Never".to_string(),
        Ty::Unit => "()".to_string(),
        Ty::Bool => "Bool".to_string(),
        Ty::Int(kind) => display_int_kind(*kind).to_string(),
        Ty::Float => "Float".to_string(),
        Ty::Str => "Str".to_string(),
        Ty::String => "String".to_string(),
        Ty::RawPtr { mutable } => {
            if *mutable {
                "*mut _".to_string()
            } else {
                "*const _".to_string()
            }
        }
        Ty::Fn {
            params,
            ret,
            is_async,
        } => {
            let params = params.iter().map(display_ty).collect::<Vec<_>>().join(", ");
            let prefix = if *is_async { "async fn" } else { "fn" };
            format!("{prefix}({params}) -> {}", display_ty(ret))
        }
        Ty::Generic { base, args } => {
            let args = args.iter().map(display_ty).collect::<Vec<_>>().join(", ");
            format!("{base}<{args}>")
        }
        Ty::Named(name) => name.clone(),
    }
}

fn display_int_kind(kind: IntKind) -> &'static str {
    match kind {
        IntKind::Untyped => "integer literal",
        IntKind::Int => "Int",
        IntKind::Int8 => "Int8",
        IntKind::Int16 => "Int16",
        IntKind::Int32 => "Int32",
        IntKind::Int64 => "Int64",
        IntKind::UInt => "UInt",
        IntKind::UInt8 => "UInt8",
        IntKind::UInt16 => "UInt16",
        IntKind::UInt32 => "UInt32",
        IntKind::UInt64 => "UInt64",
    }
}

fn default_local_ty(ty: Ty) -> Ty {
    match ty {
        Ty::Int(IntKind::Untyped) => Ty::Int(IntKind::Int),
        other => other,
    }
}

fn equality_comparable(left: &Ty, right: &Ty) -> bool {
    if matches!(left, Ty::Unknown) || matches!(right, Ty::Unknown) {
        return true;
    }
    if matches!((left, right), (Ty::Int(_), Ty::Int(_))) {
        return true;
    }
    if matches!((left, right), (Ty::Float, Ty::Float)) {
        return true;
    }
    matches!((left, right), (Ty::Bool, Ty::Bool))
        || (is_string_like_ty(left) && is_string_like_ty(right))
}

fn is_string_like_ty(ty: &Ty) -> bool {
    matches!(ty, Ty::Str | Ty::String)
}

fn is_int_like_ty(ty: &Ty) -> bool {
    matches!(ty, Ty::Unknown | Ty::Int(_))
}

fn numeric_ty(ty: &Ty) -> bool {
    matches!(ty, Ty::Unknown | Ty::Int(_) | Ty::Float)
}

fn ordered_comparable(left: &Ty, right: &Ty) -> bool {
    if matches!(left, Ty::Unknown) || matches!(right, Ty::Unknown) {
        return true;
    }
    matches!(
        (left, right),
        (Ty::Int(_), Ty::Int(_)) | (Ty::Float, Ty::Float)
    )
}

fn int_assignable(expected: &Ty, actual: &Ty) -> bool {
    let (Ty::Int(expected), Ty::Int(actual)) = (expected, actual) else {
        return false;
    };
    if *expected == IntKind::Int {
        return true;
    }
    if *actual == IntKind::Int && matches!(expected, IntKind::Int32 | IntKind::Int64) {
        return true;
    }
    if *actual == IntKind::Untyped {
        return true;
    }
    expected == actual
}

fn raw_ptr_bridge_assignable(expected: &Ty, actual: &Ty) -> bool {
    matches!(
        (expected, actual),
        (Ty::RawPtr { .. }, Ty::Int(_)) | (Ty::Int(_), Ty::RawPtr { .. })
    ) || matches!(
        (expected, actual),
        (Ty::RawPtr { mutable: false }, Ty::RawPtr { mutable: true })
    )
}

fn receiver_assignable(expected: &Ty, actual: &Ty) -> bool {
    matches!(expected, Ty::Unknown)
        || matches!(actual, Ty::Unknown)
        || int_assignable(expected, actual)
        || raw_ptr_bridge_assignable(expected, actual)
        || string_view_assignable(expected, actual)
        || generic_assignable(expected, actual)
        || expected == actual
}

fn string_view_assignable(expected: &Ty, actual: &Ty) -> bool {
    matches!((expected, actual), (Ty::Str, Ty::String))
}

fn generic_assignable(expected: &Ty, actual: &Ty) -> bool {
    match (expected, actual) {
        (
            Ty::Generic {
                base: expected_base,
                args: expected_args,
            },
            Ty::Generic {
                base: actual_base,
                args: actual_args,
            },
        ) => generic_base_assignable(expected_base, actual_base) && expected_args == actual_args,
        (Ty::Generic { base, .. }, Ty::Named(name))
        | (Ty::Named(name), Ty::Generic { base, .. }) => generic_base_assignable(base, name),
        _ => false,
    }
}

fn generic_base_assignable(expected: &str, actual: &str) -> bool {
    expected == actual
        || expected.ends_with(&format!("__{actual}"))
        || actual.ends_with(&format!("__{expected}"))
}

fn nominal_ty_name(ty: &Ty) -> Option<String> {
    match ty {
        Ty::Named(name) => Some(name.clone()),
        Ty::Generic { base, .. } => Some(base.clone()),
        _ => None,
    }
}

fn is_byte_slice_ty(ty: &Ty) -> bool {
    nominal_ty_name(ty).is_some_and(|name| name == "ByteSlice" || name.ends_with("__ByteSlice"))
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

fn channel_type_arg_from_ty(ty: Ty) -> Option<String> {
    match ty {
        Ty::String => Some("String".to_string()),
        Ty::Bool => Some("Bool".to_string()),
        Ty::Int(_) => Some("Int".to_string()),
        Ty::Fn {
            params,
            ret,
            is_async: false,
        } if params.is_empty() && matches!(ret.as_ref(), Ty::Unit) => Some("fn".to_string()),
        Ty::Generic { base, args } if base.ends_with("Channel") => {
            args.into_iter().next().and_then(channel_type_arg_from_ty)
        }
        _ => None,
    }
}

fn infer_channel_call_type_arg(
    name: &str,
    expr: &CallExpr,
    scope: &mut HashMap<String, Ty>,
    checker: &mut TypeChecker,
    expected_channel_type_arg: Option<&str>,
) -> Option<String> {
    match channel_generic_method(name)? {
        "new" => expected_channel_type_arg.map(str::to_string),
        "send" => expr
            .args
            .get(1)
            .map(|arg| checker.infer_expr(arg, scope))
            .and_then(channel_type_arg_from_ty),
        "clone" | "recv" | "close" | "destroy" => expr
            .args
            .first()
            .map(|arg| checker.infer_expr(arg, scope))
            .and_then(channel_type_arg_from_ty)
            .or_else(|| expected_channel_type_arg.map(str::to_string)),
        _ => None,
    }
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
        } if params.is_empty()
            && matches!(return_type.as_ref(), TypeExpr::Tuple(items) if items.is_empty()) =>
        {
            Some("fn".to_string())
        }
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

fn display_type_expr(expr: &TypeExpr) -> String {
    match expr {
        TypeExpr::Path(path) => path
            .first()
            .cloned()
            .unwrap_or_else(|| "Unknown".to_string()),
        TypeExpr::Generic { base, args } => {
            let args = args
                .iter()
                .map(display_type_expr)
                .collect::<Vec<_>>()
                .join(", ");
            format!("{}<{args}>", display_type_expr(base))
        }
        TypeExpr::Tuple(items) if items.is_empty() => "()".to_string(),
        TypeExpr::Fn { .. } => "fn".to_string(),
        TypeExpr::Ref { inner, .. }
        | TypeExpr::RawPtr { inner, .. }
        | TypeExpr::Impl(inner)
        | TypeExpr::Mut(inner) => display_type_expr(inner),
        TypeExpr::Missing | TypeExpr::Tuple(_) => "Unknown".to_string(),
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

fn is_unit_function_type_arg(value: &str) -> bool {
    let normalized = value.split_whitespace().collect::<String>().to_lowercase();
    normalized == "fn()->()" || normalized == "fn"
}

fn int_literal_value(expr: &Expr) -> Option<i128> {
    match expr {
        Expr::Literal(Literal::Int(value)) => value.parse().ok(),
        Expr::Unary(expr) if matches!(expr.op, UnaryOp::Neg) => {
            int_literal_value(&expr.expr).map(|value| -value)
        }
        _ => None,
    }
}

fn int_literal_fits(kind: IntKind, value: i128) -> bool {
    let (min, max) = int_kind_range(kind);
    value >= min && value <= max
}

fn int_kind_range(kind: IntKind) -> (i128, i128) {
    match kind {
        IntKind::Untyped | IntKind::Int | IntKind::Int64 => {
            (-9_223_372_036_854_775_808, 9_223_372_036_854_775_807)
        }
        IntKind::UInt | IntKind::UInt64 => (0, 18_446_744_073_709_551_615),
        IntKind::Int8 => (-128, 127),
        IntKind::Int16 => (-32_768, 32_767),
        IntKind::Int32 => (-2_147_483_648, 2_147_483_647),
        IntKind::UInt8 => (0, 255),
        IntKind::UInt16 => (0, 65_535),
        IntKind::UInt32 => (0, 4_294_967_295),
    }
}

fn types_compatible_for_interface(expected: &Ty, actual: &Ty) -> bool {
    matches!(expected, Ty::Unknown)
        || matches!(actual, Ty::Unknown)
        || matches!(expected, Ty::Named(name) if name == "Self")
        || int_assignable(expected, actual)
        || string_view_assignable(expected, actual)
        || expected == actual
}

fn normalize_param_name(name: &str) -> String {
    match name {
        "self" | "&self" | "&mut self" => "self".to_string(),
        _ => name.to_string(),
    }
}

fn pattern_bindings(pattern: &str) -> Vec<String> {
    pattern
        .split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
        .filter(|part| !part.is_empty())
        .filter(|part| {
            part.chars()
                .next()
                .is_some_and(|ch| ch.is_ascii_lowercase() || ch == '_')
        })
        .filter(|part| *part != "_")
        .map(str::to_string)
        .collect()
}

fn pattern_payload_count(pattern: &str) -> usize {
    let Some(open) = pattern.find('(') else {
        return 0;
    };
    let Some(close) = pattern.rfind(')') else {
        return 0;
    };
    let payload = pattern[open + 1..close].trim();
    if payload.is_empty() {
        return 0;
    }
    payload
        .split(',')
        .filter(|part| !part.trim().is_empty())
        .count()
}

fn pattern_variant(pattern: &str) -> Option<String> {
    let first = pattern
        .split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
        .find(|part| !part.is_empty())?;
    if first
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_uppercase())
    {
        Some(first.to_string())
    } else {
        None
    }
}

fn starts_with_lowercase_binding(pattern: &str) -> bool {
    pattern
        .chars()
        .find(|ch| !ch.is_whitespace())
        .is_some_and(|ch| ch.is_ascii_lowercase())
}
