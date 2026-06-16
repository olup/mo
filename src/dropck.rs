use std::collections::HashMap;

use crate::ast::*;
use crate::hir::{HirFunction, HirProgram};
use crate::semantics::Diagnostic;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DropReport {
    pub checked_functions: usize,
    pub function_drops: HashMap<String, Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum DropTy {
    Unknown,
    Unit,
    Bool,
    Int,
    Float,
    Char,
    String,
    Named(String),
    Ref,
}

#[derive(Debug, Clone)]
struct LocalDrop {
    name: String,
    ty: DropTy,
    moved: bool,
}

#[derive(Debug, Clone)]
struct FunctionSig {
    params: Vec<ParamMode>,
    ret: DropTy,
}

#[derive(Debug, Clone)]
struct ParamMode {
    by_ref: bool,
    ty: DropTy,
}

pub fn check_drops(program: &HirProgram) -> Result<DropReport, Vec<Diagnostic>> {
    let mut checker = DropChecker::new(program);
    checker.check_program(program);
    checker.finish()
}

struct DropChecker {
    functions: HashMap<String, FunctionSig>,
    enum_variants: std::collections::HashSet<String>,
    diagnostics: Vec<Diagnostic>,
    checked_functions: usize,
    function_drops: HashMap<String, Vec<String>>,
}

impl DropChecker {
    fn new(program: &HirProgram) -> Self {
        let mut functions: HashMap<_, _> = program
            .functions
            .iter()
            .map(|function| {
                (
                    function.name.clone(),
                    FunctionSig {
                        params: function
                            .params
                            .iter()
                            .map(|param| {
                                param.ty_expr.as_ref().map(param_mode).unwrap_or(ParamMode {
                                    by_ref: false,
                                    ty: DropTy::Unknown,
                                })
                            })
                            .collect(),
                        ret: function
                            .return_type
                            .as_ref()
                            .map(drop_ty_from_type)
                            .unwrap_or(DropTy::Unit),
                    },
                )
            })
            .collect();
        for function in &program.extern_functions {
            functions.insert(
                function.name.clone(),
                FunctionSig {
                    params: function
                        .params
                        .iter()
                        .map(|param| {
                            param.ty_expr.as_ref().map(param_mode).unwrap_or(ParamMode {
                                by_ref: false,
                                ty: DropTy::Unknown,
                            })
                        })
                        .collect(),
                    ret: function
                        .return_type
                        .as_ref()
                        .map(drop_ty_from_type)
                        .unwrap_or(DropTy::Unit),
                },
            );
        }

        Self {
            functions,
            enum_variants: program
                .enums
                .iter()
                .flat_map(|item| item.variants.iter().map(|variant| variant.name.clone()))
                .collect(),
            diagnostics: Vec::new(),
            checked_functions: 0,
            function_drops: HashMap::new(),
        }
    }

    fn check_program(&mut self, program: &HirProgram) {
        for function in &program.functions {
            self.check_function(function);
        }
        for test in &program.tests {
            let mut scope = Vec::new();
            self.check_block(&test.body, &mut scope);
            self.function_drops
                .insert(format!("test {}", test.name), planned_drops(&scope));
        }
    }

    fn check_function(&mut self, function: &HirFunction) {
        self.checked_functions += 1;
        let mut scope = Vec::new();
        for param in &function.params {
            let ty = param
                .ty_expr
                .as_ref()
                .map(drop_ty_from_type)
                .unwrap_or(DropTy::Unknown);
            scope.push(LocalDrop {
                name: normalize_param_name(&param.name),
                ty,
                moved: false,
            });
        }
        self.check_block(&function.body, &mut scope);
        self.function_drops
            .insert(function.name.clone(), planned_drops(&scope));
    }

    fn check_block(&mut self, block: &Block, scope: &mut Vec<LocalDrop>) {
        for stmt in &block.statements {
            self.check_stmt(stmt, scope);
        }
    }

    fn check_child_block(&mut self, block: &Block, parent: &[LocalDrop]) {
        let mut child = parent.to_vec();
        self.check_block(block, &mut child);
    }

    fn check_stmt(&mut self, stmt: &Stmt, scope: &mut Vec<LocalDrop>) {
        match &stmt.data {
            StmtData::Let(stmt) => {
                let inferred = stmt.value.as_ref().map(|value| {
                    let ty = self.infer_expr_ty(value, scope);
                    self.consume_expr(value, scope);
                    ty
                });
                let ty = stmt
                    .ty_expr
                    .as_ref()
                    .map(drop_ty_from_type)
                    .or(inferred)
                    .unwrap_or(DropTy::Unknown);
                scope.push(LocalDrop {
                    name: stmt.name.clone(),
                    ty,
                    moved: false,
                });
            }
            StmtData::Return(expr) | StmtData::Break(expr) => {
                if let Some(expr) = expr {
                    self.consume_expr(expr, scope);
                }
            }
            StmtData::Continue | StmtData::Raw => {}
            StmtData::Expr(expr) => {
                if let Expr::Binary(binary) = expr {
                    if binary.op.is_assignment() {
                        self.read_expr(&binary.left, scope);
                        self.consume_expr(&binary.right, scope);
                        return;
                    }
                }
                if let Expr::Call(call) = expr {
                    if is_vec_push_call(&call.callee) {
                        self.consume_vec_push(call, scope);
                        return;
                    }
                    if is_map_put_call(&call.callee) {
                        self.consume_map_put(call, scope);
                        return;
                    }
                    if self.call_consumes_first_arg(call, scope) {
                        self.consume_call(call, scope);
                        return;
                    }
                }
                self.read_expr(expr, scope);
            }
            StmtData::If(control) | StmtData::While(control) => {
                if let Some(condition) = &control.condition {
                    self.read_expr(condition, scope);
                }
                self.check_child_block(&control.body, scope);
            }
            StmtData::Match(expr) => self.read_match(expr, scope),
            StmtData::For(stmt) => {
                self.read_expr(&stmt.iterator, scope);
                let mut child = scope.clone();
                for binding in pattern_bindings(&stmt.pattern) {
                    child.push(LocalDrop {
                        name: binding,
                        ty: DropTy::Unknown,
                        moved: false,
                    });
                }
                self.check_block(&stmt.body, &mut child);
            }
            StmtData::Loop(block) | StmtData::Unsafe(block) => self.check_child_block(block, scope),
        }
    }

    fn consume_expr(&mut self, expr: &Expr, scope: &mut Vec<LocalDrop>) {
        match expr {
            Expr::Ident(name) => self.move_local(name, scope),
            Expr::Unary(expr) => match expr.op {
                UnaryOp::Ref | UnaryOp::MutRef | UnaryOp::Deref | UnaryOp::Neg | UnaryOp::Not => {
                    self.read_expr(&expr.expr, scope);
                }
            },
            Expr::Mut(expr) | Expr::Await(expr) | Expr::Try(expr) => self.read_expr(expr, scope),
            Expr::Binary(expr) => {
                self.read_expr(&expr.left, scope);
                self.consume_expr(&expr.right, scope);
            }
            Expr::Call(expr) => self.consume_call(expr, scope),
            Expr::Struct(expr) => {
                for field in &expr.fields {
                    if let Some(value) = &field.value {
                        self.consume_expr(value, scope);
                    }
                }
            }
            Expr::Block(block) => {
                let child = self.check_block_expr_scope(block, scope, true);
                propagate_child_moves(scope, &child);
            }
            Expr::Match(expr) => {
                let ty = self.infer_match_ty(expr, scope);
                if !matches!(ty, DropTy::Unknown) && is_copy_ty(&ty) {
                    self.read_match(expr, scope);
                } else {
                    self.consume_match(expr, scope);
                }
            }
            _ => self.read_expr(expr, scope),
        }
    }

    fn read_expr(&mut self, expr: &Expr, scope: &mut Vec<LocalDrop>) {
        match expr {
            Expr::Missing | Expr::Ident(_) | Expr::Path(_) | Expr::Literal(_) | Expr::Raw(_) => {}
            Expr::Unary(expr) => self.read_expr(&expr.expr, scope),
            Expr::Mut(expr) | Expr::Await(expr) | Expr::Try(expr) => self.read_expr(expr, scope),
            Expr::Binary(expr) => {
                self.read_expr(&expr.left, scope);
                self.read_expr(&expr.right, scope);
            }
            Expr::Index(expr) => {
                self.read_expr(&expr.target, scope);
                self.read_expr(&expr.index, scope);
            }
            Expr::Call(expr) => self.consume_call(expr, scope),
            Expr::Member(expr) => self.read_expr(&expr.target, scope),
            Expr::Struct(expr) => {
                for field in &expr.fields {
                    if let Some(value) = &field.value {
                        self.read_expr(value, scope);
                    }
                }
            }
            Expr::Object(expr) => {
                for field in &expr.fields {
                    self.read_expr(&field.value, scope);
                }
            }
            Expr::Closure(expr) => {
                let captures = expr.is_move.then(|| closure_captures(expr, scope));
                let mut child = scope.clone();
                if let Some(captures) = captures {
                    for capture in captures {
                        self.move_local(&capture, scope);
                    }
                }
                for param in &expr.params {
                    let ty = param
                        .ty_expr
                        .as_ref()
                        .map(drop_ty_from_type)
                        .unwrap_or(DropTy::Unknown);
                    child.push(LocalDrop {
                        name: normalize_param_name(&param.name),
                        ty,
                        moved: false,
                    });
                }
                self.check_block(&expr.body, &mut child);
            }
            Expr::Match(expr) => self.read_match(expr, scope),
            Expr::If(expr) => {
                self.read_expr(&expr.condition, scope);
                self.check_child_block(&expr.then_branch, scope);
                if let Some(else_branch) = &expr.else_branch {
                    self.check_child_block(else_branch, scope);
                }
            }
            Expr::Block(block) => {
                let child = self.check_block_expr_scope(block, scope, false);
                propagate_child_moves(scope, &child);
            }
        }
    }

    fn check_block_expr_scope(
        &mut self,
        block: &Block,
        scope: &[LocalDrop],
        consume_final_expr: bool,
    ) -> Vec<LocalDrop> {
        let mut child = scope.to_vec();
        let final_expr_index = block
            .statements
            .last()
            .filter(|stmt| matches!(stmt.data, StmtData::Expr(_)))
            .map(|_| block.statements.len() - 1);
        for (index, stmt) in block.statements.iter().enumerate() {
            if Some(index) == final_expr_index {
                if let StmtData::Expr(expr) = &stmt.data {
                    if consume_final_expr {
                        self.consume_expr(expr, &mut child);
                    } else {
                        self.read_expr(expr, &mut child);
                    }
                }
                break;
            }
            self.check_stmt(stmt, &mut child);
        }
        child
    }

    fn consume_vec_push(&mut self, expr: &CallExpr, scope: &mut Vec<LocalDrop>) {
        self.read_expr(&expr.callee, scope);
        if let Some(vec) = expr.args.first() {
            self.read_expr(vec, scope);
        }
        if let Some(value) = expr.args.get(1) {
            self.consume_expr(value, scope);
        }
        for arg in expr.args.iter().skip(2) {
            self.read_expr(arg, scope);
        }
    }

    fn consume_map_put(&mut self, expr: &CallExpr, scope: &mut Vec<LocalDrop>) {
        self.read_expr(&expr.callee, scope);
        if is_alloc_map_put_call(&expr.callee) {
            for arg in expr.args.iter().take(2) {
                self.read_expr(arg, scope);
            }
            for value in expr.args.iter().skip(2).take(2) {
                self.consume_expr(value, scope);
            }
            for arg in expr.args.iter().skip(4) {
                self.read_expr(arg, scope);
            }
        } else {
            if let Some(map) = expr.args.first() {
                self.read_expr(map, scope);
            }
            for value in expr.args.iter().skip(1).take(2) {
                self.consume_expr(value, scope);
            }
            for arg in expr.args.iter().skip(3) {
                self.read_expr(arg, scope);
            }
        }
    }

    fn consume_call(&mut self, expr: &CallExpr, scope: &mut Vec<LocalDrop>) {
        if is_vec_push_call(&expr.callee) {
            self.consume_vec_push(expr, scope);
            return;
        }
        if is_map_put_call(&expr.callee) {
            self.consume_map_put(expr, scope);
            return;
        }
        if self.call_consumes_first_arg(expr, scope) {
            self.read_expr(&expr.callee, scope);
            if let Some(arg) = expr.args.first() {
                self.consume_expr(arg, scope);
            }
            for arg in expr.args.iter().skip(1) {
                self.read_expr(arg, scope);
            }
            return;
        }
        if is_std_string_read_call(&expr.callee) {
            self.read_expr(&expr.callee, scope);
            for arg in &expr.args {
                self.read_expr(arg, scope);
            }
            return;
        }
        if direct_callee_name(&expr.callee).is_some_and(|name| self.enum_variants.contains(name)) {
            for arg in &expr.args {
                self.consume_expr(arg, scope);
            }
            return;
        }

        let sig = direct_callee_name(&expr.callee)
            .and_then(|name| self.functions.get(name))
            .cloned();

        if let Some(sig) = sig {
            for (arg, param) in expr.args.iter().zip(&sig.params) {
                if param.by_ref || is_copy_ty(&param.ty) {
                    self.read_expr(arg, scope);
                } else {
                    self.consume_expr(arg, scope);
                }
            }
            for arg in expr.args.iter().skip(sig.params.len()) {
                self.read_expr(arg, scope);
            }
        } else {
            self.read_expr(&expr.callee, scope);
            for arg in &expr.args {
                self.read_expr(arg, scope);
            }
        }
    }

    fn call_consumes_first_arg(&self, expr: &CallExpr, scope: &[LocalDrop]) -> bool {
        if is_explicit_resource_destroy_call(&expr.callee) || is_box_new_call(&expr.callee) {
            return true;
        }
        let Some(first) = expr.args.first() else {
            return false;
        };
        let first_ty = self.infer_expr_ty(first, scope);
        is_resource_cleanup_member_call(&expr.callee, &first_ty)
    }

    fn read_match(&mut self, expr: &MatchExpr, scope: &mut Vec<LocalDrop>) {
        self.read_expr(&expr.value, scope);
        for arm in &expr.arms {
            self.read_expr(&arm.body, scope);
        }
    }

    fn consume_match(&mut self, expr: &MatchExpr, scope: &mut Vec<LocalDrop>) {
        self.consume_expr(&expr.value, scope);
        for arm in &expr.arms {
            self.consume_expr(&arm.body, scope);
        }
    }

    fn infer_expr_ty(&self, expr: &Expr, scope: &[LocalDrop]) -> DropTy {
        match expr {
            Expr::Missing | Expr::Path(_) | Expr::Raw(_) => DropTy::Unknown,
            Expr::Ident(name) => find_local(name, scope)
                .map(|local| local.ty.clone())
                .or_else(|| self.functions.get(name).map(|sig| sig.ret.clone()))
                .unwrap_or(DropTy::Unknown),
            Expr::Literal(literal) => match literal {
                Literal::Int(_) => DropTy::Int,
                Literal::Float(_) => DropTy::Float,
                Literal::String(_) => DropTy::String,
                Literal::Char(_) => DropTy::Char,
                Literal::Bool(_) => DropTy::Bool,
                Literal::Unit => DropTy::Unit,
            },
            Expr::Unary(expr) => match expr.op {
                UnaryOp::Ref | UnaryOp::MutRef => DropTy::Ref,
                _ => self.infer_expr_ty(&expr.expr, scope),
            },
            Expr::Mut(expr) | Expr::Await(expr) | Expr::Try(expr) => {
                self.infer_expr_ty(expr, scope)
            }
            Expr::Binary(expr) => match expr.op {
                BinaryOp::Eq
                | BinaryOp::NotEq
                | BinaryOp::Lt
                | BinaryOp::Le
                | BinaryOp::Gt
                | BinaryOp::Ge
                | BinaryOp::BoolAnd
                | BinaryOp::BoolOr => DropTy::Bool,
                BinaryOp::Assign
                | BinaryOp::AddAssign
                | BinaryOp::SubAssign
                | BinaryOp::MulAssign
                | BinaryOp::DivAssign
                | BinaryOp::RemAssign
                | BinaryOp::BitAndAssign
                | BinaryOp::BitOrAssign
                | BinaryOp::ShlAssign
                | BinaryOp::ShrAssign => DropTy::Unit,
                BinaryOp::Add
                | BinaryOp::Sub
                | BinaryOp::Mul
                | BinaryOp::Div
                | BinaryOp::Rem
                | BinaryOp::BitAnd
                | BinaryOp::BitOr
                | BinaryOp::BitXor
                | BinaryOp::Shl
                | BinaryOp::Shr => DropTy::Int,
            },
            Expr::Index(_) => DropTy::Int,
            Expr::Call(expr) => direct_callee_name(&expr.callee)
                .and_then(|name| self.functions.get(name))
                .map(|sig| sig.ret.clone())
                .unwrap_or(DropTy::Unknown),
            Expr::Struct(expr) => DropTy::Named(expr.name.clone()),
            Expr::Object(_) => DropTy::Unknown,
            Expr::Block(block) => self.infer_block_ty(block, scope),
            Expr::Match(expr) => self.infer_match_ty(expr, scope),
            Expr::If(expr) => self.infer_if_ty(expr, scope),
            Expr::Member(_) | Expr::Closure(_) => DropTy::Unknown,
        }
    }

    fn infer_if_ty(&self, expr: &IfExpr, scope: &[LocalDrop]) -> DropTy {
        let Some(else_branch) = &expr.else_branch else {
            return DropTy::Unknown;
        };
        let then_ty = self.infer_block_ty(&expr.then_branch, scope);
        let else_ty = self.infer_block_ty(else_branch, scope);
        if then_ty == else_ty {
            then_ty
        } else {
            DropTy::Unknown
        }
    }

    fn infer_match_ty(&self, expr: &MatchExpr, scope: &[LocalDrop]) -> DropTy {
        let mut arms = expr.arms.iter();
        let Some(first) = arms.next() else {
            return DropTy::Unknown;
        };
        let first_ty = self.infer_expr_ty(&first.body, scope);
        if arms.all(|arm| self.infer_expr_ty(&arm.body, scope) == first_ty) {
            first_ty
        } else {
            DropTy::Unknown
        }
    }

    fn infer_block_ty(&self, block: &Block, scope: &[LocalDrop]) -> DropTy {
        let mut child = scope.to_vec();
        for stmt in &block.statements {
            match &stmt.data {
                StmtData::Let(stmt) => {
                    let ty = stmt
                        .ty_expr
                        .as_ref()
                        .map(drop_ty_from_type)
                        .or_else(|| {
                            stmt.value
                                .as_ref()
                                .map(|value| self.infer_expr_ty(value, &child))
                        })
                        .unwrap_or(DropTy::Unknown);
                    child.push(LocalDrop {
                        name: stmt.name.clone(),
                        ty,
                        moved: false,
                    });
                }
                StmtData::Expr(expr) => return self.infer_expr_ty(expr, &child),
                _ => {}
            }
        }
        DropTy::Unknown
    }

    fn move_local(&mut self, name: &str, scope: &mut [LocalDrop]) {
        let Some(local) = find_local_mut(name, scope) else {
            return;
        };
        if !is_copy_ty(&local.ty) {
            local.moved = true;
        }
    }

    fn finish(self) -> Result<DropReport, Vec<Diagnostic>> {
        if self.diagnostics.is_empty() {
            Ok(DropReport {
                checked_functions: self.checked_functions,
                function_drops: self.function_drops,
            })
        } else {
            Err(self.diagnostics)
        }
    }
}

fn planned_drops(scope: &[LocalDrop]) -> Vec<String> {
    scope
        .iter()
        .rev()
        .filter(|local| !local.moved && needs_drop(&local.ty))
        .map(|local| local.name.clone())
        .collect()
}

fn propagate_child_moves(scope: &mut [LocalDrop], child: &[LocalDrop]) {
    for (local, child_local) in scope.iter_mut().zip(child) {
        if child_local.moved {
            local.moved = true;
        }
    }
}

fn find_local<'a>(name: &str, scope: &'a [LocalDrop]) -> Option<&'a LocalDrop> {
    scope.iter().rev().find(|local| local.name == name)
}

fn find_local_mut<'a>(name: &str, scope: &'a mut [LocalDrop]) -> Option<&'a mut LocalDrop> {
    scope.iter_mut().rev().find(|local| local.name == name)
}

fn param_mode(expr: &TypeExpr) -> ParamMode {
    match expr {
        TypeExpr::Ref { inner, .. } => ParamMode {
            by_ref: true,
            ty: drop_ty_from_type(inner),
        },
        TypeExpr::RawPtr { .. } => ParamMode {
            by_ref: false,
            ty: DropTy::Int,
        },
        _ => ParamMode {
            by_ref: false,
            ty: drop_ty_from_type(expr),
        },
    }
}

fn drop_ty_from_type(expr: &TypeExpr) -> DropTy {
    match expr {
        TypeExpr::Path(path) => path
            .first()
            .map(|name| match name.as_str() {
                "Bool" => DropTy::Bool,
                "Int" | "Int8" | "Int16" | "Int32" | "Int64" | "UInt" | "UInt8" | "UInt16"
                | "UInt32" | "UInt64" => DropTy::Int,
                "Float32" | "Float64" => DropTy::Float,
                "Char" => DropTy::Char,
                "String" | "Str" => DropTy::String,
                other => DropTy::Named(other.to_string()),
            })
            .unwrap_or(DropTy::Unknown),
        TypeExpr::Tuple(items) if items.is_empty() => DropTy::Unit,
        TypeExpr::Tuple(_) => DropTy::Unknown,
        TypeExpr::Generic { base, args } => {
            if args.iter().any(type_contains_generic_placeholder) {
                DropTy::Unknown
            } else {
                drop_ty_from_type(base)
            }
        }
        TypeExpr::Ref { .. } => DropTy::Ref,
        TypeExpr::RawPtr { .. } => DropTy::Int,
        TypeExpr::Impl(_) | TypeExpr::Fn { .. } => DropTy::Unknown,
        TypeExpr::Mut(inner) => drop_ty_from_type(inner),
        TypeExpr::Missing => DropTy::Unknown,
    }
}

fn type_contains_generic_placeholder(expr: &TypeExpr) -> bool {
    match expr {
        TypeExpr::Path(path) => path
            .first()
            .is_some_and(|name| name.len() == 1 && name.chars().all(|ch| ch.is_ascii_uppercase())),
        TypeExpr::Generic { base, args } => {
            type_contains_generic_placeholder(base)
                || args.iter().any(type_contains_generic_placeholder)
        }
        TypeExpr::Tuple(items) => items.iter().any(type_contains_generic_placeholder),
        TypeExpr::Ref { inner, .. } | TypeExpr::Mut(inner) => {
            type_contains_generic_placeholder(inner)
        }
        TypeExpr::Fn {
            params,
            return_type,
            ..
        } => {
            params.iter().any(type_contains_generic_placeholder)
                || type_contains_generic_placeholder(return_type)
        }
        TypeExpr::RawPtr { inner, .. } => type_contains_generic_placeholder(inner),
        TypeExpr::Impl(_) | TypeExpr::Missing => false,
    }
}

fn is_copy_ty(ty: &DropTy) -> bool {
    matches!(
        ty,
        DropTy::Unknown
            | DropTy::Unit
            | DropTy::Bool
            | DropTy::Int
            | DropTy::Float
            | DropTy::Char
            | DropTy::Ref
    )
}

fn needs_drop(ty: &DropTy) -> bool {
    matches!(ty, DropTy::String | DropTy::Named(_))
}

fn direct_callee_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Ident(name) => Some(name),
        Expr::Path(path) => path.last().map(String::as_str),
        _ => None,
    }
}

fn is_resource_cleanup_member_call(expr: &Expr, first_arg_ty: &DropTy) -> bool {
    matches!(
        (expr, first_arg_ty),
        (
            Expr::Member(MemberExpr { member, .. }) | Expr::Ident(member),
            DropTy::Named(name),
        ) if member == "destroy" && matches!(name.as_str(), "Channel" | "channel__Channel")
    ) || matches!(
        (expr, first_arg_ty),
        (
            Expr::Member(MemberExpr { member, .. }) | Expr::Ident(member),
            DropTy::Named(name),
        ) if matches!(member.as_str(), "destroy" | "finish") && name == "Buffer"
    ) || matches!(
        (expr, first_arg_ty),
        (
            Expr::Member(MemberExpr { member, .. }) | Expr::Ident(member),
            DropTy::Named(name),
        ) if member == "destroy" && name == "Box"
    ) || matches!(
        (expr, first_arg_ty),
        (
            Expr::Member(MemberExpr { member, .. }) | Expr::Ident(member),
            DropTy::Named(name),
        ) if member == "destroy" && name == "Vec"
    ) || matches!(
        (expr, first_arg_ty),
        (
            Expr::Member(MemberExpr { member, .. }) | Expr::Ident(member),
            DropTy::Named(name),
        ) if member == "destroy" && name == "Map"
    ) || matches!(
        (expr, first_arg_ty),
        (
            Expr::Member(MemberExpr { member, .. }) | Expr::Ident(member),
            DropTy::Named(name),
        ) if member == "tcp_listener_close" && matches!(name.as_str(), "TcpListener" | "net__TcpListener")
    ) || matches!(
        (expr, first_arg_ty),
        (
            Expr::Member(MemberExpr { member, .. }) | Expr::Ident(member),
            DropTy::Named(name),
        ) if member == "tcp_stream_close" && matches!(name.as_str(), "TcpStream" | "net__TcpStream")
    )
}

fn is_vec_push_call(expr: &Expr) -> bool {
    direct_callee_name(expr).is_some_and(is_vec_push_name)
        || matches!(
            expr,
            Expr::Member(MemberExpr { target, member })
                if matches!(member.as_str(), "push" | "push_int" | "push_string")
                    && matches!(target.as_ref(), Expr::Ident(name) if name == "vec")
        )
}

fn is_vec_push_name(name: &str) -> bool {
    name.ends_with("vec__push") || name.starts_with("vec__push_") || name.contains("__vec__push_")
}

fn is_map_put_call(expr: &Expr) -> bool {
    direct_callee_name(expr).is_some_and(is_map_put_name)
        || matches!(
            expr,
            Expr::Member(MemberExpr { target, member })
                if member == "put" && matches!(target.as_ref(), Expr::Ident(name) if name == "map")
        )
}

fn is_alloc_map_put_call(expr: &Expr) -> bool {
    direct_callee_name(expr).is_some_and(is_alloc_map_put_name)
}

fn is_map_put_name(name: &str) -> bool {
    name.ends_with("map__put")
        || name.starts_with("map__put_")
        || name.contains("__map__put_")
        || is_alloc_map_put_name(name)
}

fn is_alloc_map_put_name(name: &str) -> bool {
    name.ends_with("alloc_map__put_string_string")
        || name.ends_with("alloc__map__put_string_string")
}

fn is_box_new_call(expr: &Expr) -> bool {
    direct_callee_name(expr).is_some_and(|name| name.ends_with("box__new"))
        || matches!(
            expr,
            Expr::Member(MemberExpr { target, member })
                if member == "new" && matches!(target.as_ref(), Expr::Ident(name) if name == "box")
        )
}

fn is_explicit_resource_destroy_call(expr: &Expr) -> bool {
    direct_callee_name(expr).is_some_and(|name| {
        matches!(name, "free_owned" | "raw_free")
            || name.ends_with("__free_owned")
            || name.ends_with("__raw_free")
            || matches!(name, "destroy_queue" | "destroy_queue_int")
            || name.ends_with("__destroy_queue")
            || name.ends_with("__destroy_queue_int")
            || name.ends_with("buffer__destroy")
            || name.ends_with("buffer__finish")
            || name.ends_with("buffer__string_builder_destroy")
            || name.ends_with("buffer__string_builder_finish")
            || name.ends_with("buffer__byte_buffer_destroy")
            || name.ends_with("buffer__byte_buffer_finish")
            || name.ends_with("box__destroy_int")
            || name.ends_with("http__headers_destroy")
            || name.ends_with("http__request_destroy")
            || name.ends_with("http__response_destroy")
            || name.ends_with("map__destroy")
            || name.ends_with("map__destroy_string_string")
            || name.ends_with("vec__destroy")
            || name.ends_with("vec__destroy_int")
            || name.ends_with("vec__destroy_string")
            || name.ends_with("net__tcp_listener_close")
            || name.ends_with("net__tcp_stream_close")
            || matches!(name, "tcp_listener_close" | "tcp_stream_close")
    }) || matches!(
        expr,
        Expr::Member(MemberExpr { target, member })
            if member == "free_owned"
                && matches!(target.as_ref(), Expr::Ident(name) if name == "String")

    ) || matches!(
        expr,
        Expr::Member(MemberExpr { target, member })
            if matches!(member.as_str(), "destroy" | "finish")
                && matches!(target.as_ref(), Expr::Ident(name) if name == "buffer")
    ) || matches!(
        expr,
        Expr::Member(MemberExpr { target, member })
            if member == "destroy"
                && matches!(target.as_ref(), Expr::Ident(name) if name == "box")
    ) || matches!(
        expr,
        Expr::Member(MemberExpr { target, member })
            if member == "destroy"
                && matches!(target.as_ref(), Expr::Ident(name) if name == "vec")
    ) || matches!(
        expr,
        Expr::Member(MemberExpr { target, member })
            if member == "destroy"
                && matches!(target.as_ref(), Expr::Ident(name) if name == "map")
    )
}

fn is_std_string_read_call(expr: &Expr) -> bool {
    direct_callee_name(expr).is_some_and(|name| {
        matches!(
            name,
            "len"
                | "concat"
                | "string_load8"
                | "string_store8"
                | "getcwd"
                | "_NSGetExecutablePath"
                | "read"
        ) || name.ends_with("__len")
            || name.ends_with("__concat")
            || name.ends_with("__string_load8")
            || name.ends_with("__string_store8")
            || name.ends_with("__getcwd")
            || name.ends_with("___NSGetExecutablePath")
            || name.ends_with("__read")
    }) || matches!(
        expr,
        Expr::Member(MemberExpr { target, member })
            if matches!(member.as_str(), "len" | "concat")
                && matches!(target.as_ref(), Expr::Ident(name) if name == "String")
    )
}

fn normalize_param_name(name: &str) -> String {
    match name {
        "self" | "&self" | "&mut self" => "self".to_string(),
        _ => name.to_string(),
    }
}

fn closure_captures(expr: &ClosureExpr, scope: &[LocalDrop]) -> Vec<String> {
    let mut locals = expr
        .params
        .iter()
        .map(|param| normalize_param_name(&param.name))
        .collect::<Vec<_>>();
    let mut captures = Vec::new();
    collect_block_captures(&expr.body, scope, &mut locals, &mut captures);
    captures.sort();
    captures.dedup();
    captures
}

fn collect_block_captures(
    block: &Block,
    scope: &[LocalDrop],
    locals: &mut Vec<String>,
    captures: &mut Vec<String>,
) {
    for stmt in &block.statements {
        if let StmtData::Let(stmt) = &stmt.data {
            locals.push(stmt.name.clone());
        }
        collect_stmt_captures(stmt, scope, locals, captures);
    }
}

fn collect_stmt_captures(
    stmt: &Stmt,
    scope: &[LocalDrop],
    locals: &mut Vec<String>,
    captures: &mut Vec<String>,
) {
    match &stmt.data {
        StmtData::Let(stmt) => {
            if let Some(value) = &stmt.value {
                collect_expr_captures(value, scope, locals, captures);
            }
        }
        StmtData::Return(expr) | StmtData::Break(expr) => {
            if let Some(expr) = expr {
                collect_expr_captures(expr, scope, locals, captures);
            }
        }
        StmtData::Continue | StmtData::Raw => {}
        StmtData::If(control) | StmtData::While(control) => {
            if let Some(condition) = &control.condition {
                collect_expr_captures(condition, scope, locals, captures);
            }
            collect_block_captures(&control.body, scope, &mut locals.clone(), captures);
        }
        StmtData::Match(expr) => collect_match_captures(expr, scope, locals, captures),
        StmtData::For(stmt) => {
            collect_expr_captures(&stmt.iterator, scope, locals, captures);
            let mut child = locals.clone();
            for binding in pattern_bindings(&stmt.pattern) {
                child.push(binding);
            }
            collect_block_captures(&stmt.body, scope, &mut child, captures);
        }
        StmtData::Loop(block) | StmtData::Unsafe(block) => {
            collect_block_captures(block, scope, &mut locals.clone(), captures);
        }
        StmtData::Expr(expr) => collect_expr_captures(expr, scope, locals, captures),
    }
}

fn collect_expr_captures(
    expr: &Expr,
    scope: &[LocalDrop],
    locals: &mut Vec<String>,
    captures: &mut Vec<String>,
) {
    match expr {
        Expr::Ident(name) => {
            if find_local(name, scope).is_some() && !locals.contains(name) {
                captures.push(name.clone());
            }
        }
        Expr::Unary(expr) => collect_expr_captures(&expr.expr, scope, locals, captures),
        Expr::Mut(expr) | Expr::Await(expr) | Expr::Try(expr) => {
            collect_expr_captures(expr, scope, locals, captures);
        }
        Expr::Binary(expr) => {
            collect_expr_captures(&expr.left, scope, locals, captures);
            collect_expr_captures(&expr.right, scope, locals, captures);
        }
        Expr::Index(expr) => {
            collect_expr_captures(&expr.target, scope, locals, captures);
            collect_expr_captures(&expr.index, scope, locals, captures);
        }
        Expr::Call(expr) => {
            collect_expr_captures(&expr.callee, scope, locals, captures);
            for arg in &expr.args {
                collect_expr_captures(arg, scope, locals, captures);
            }
        }
        Expr::Member(expr) => collect_expr_captures(&expr.target, scope, locals, captures),
        Expr::Struct(expr) => {
            for field in &expr.fields {
                if let Some(value) = &field.value {
                    collect_expr_captures(value, scope, locals, captures);
                }
            }
        }
        Expr::Object(expr) => {
            for field in &expr.fields {
                collect_expr_captures(&field.value, scope, locals, captures);
            }
        }
        Expr::Closure(expr) => collect_nested_closure_captures(expr, scope, locals, captures),
        Expr::Match(expr) => collect_match_captures(expr, scope, locals, captures),
        Expr::If(expr) => {
            collect_expr_captures(&expr.condition, scope, locals, captures);
            collect_block_captures(&expr.then_branch, scope, &mut locals.clone(), captures);
            if let Some(else_branch) = &expr.else_branch {
                collect_block_captures(else_branch, scope, &mut locals.clone(), captures);
            }
        }
        Expr::Block(block) => collect_block_captures(block, scope, &mut locals.clone(), captures),
        Expr::Missing | Expr::Path(_) | Expr::Literal(_) | Expr::Raw(_) => {}
    }
}

fn collect_nested_closure_captures(
    expr: &ClosureExpr,
    scope: &[LocalDrop],
    locals: &mut [String],
    captures: &mut Vec<String>,
) {
    let mut child = locals.to_owned();
    for param in &expr.params {
        child.push(normalize_param_name(&param.name));
    }
    collect_block_captures(&expr.body, scope, &mut child, captures);
}

fn collect_match_captures(
    expr: &MatchExpr,
    scope: &[LocalDrop],
    locals: &mut Vec<String>,
    captures: &mut Vec<String>,
) {
    collect_expr_captures(&expr.value, scope, locals, captures);
    for arm in &expr.arms {
        let mut child = locals.clone();
        for binding in pattern_bindings(&arm.pattern) {
            child.push(binding);
        }
        collect_expr_captures(&arm.body, scope, &mut child, captures);
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
