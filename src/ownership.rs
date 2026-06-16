use std::collections::{HashMap, HashSet};

use crate::ast::*;
use crate::hir::{HirFunction, HirProgram};
use crate::semantics::Diagnostic;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnershipReport {
    pub checked_functions: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MoveMode {
    Read,
    Move,
    Borrow,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum OwnTy {
    Unknown,
    Unit,
    Bool,
    Int,
    Float,
    Char,
    String,
    Named(String),
    Ref,
    Function {
        params: Vec<ParamMode>,
        ret: Box<OwnTy>,
    },
}

#[derive(Debug, Clone)]
struct FunctionSig {
    params: Vec<ParamMode>,
    ret: OwnTy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParamMode {
    by_ref: bool,
    ty: OwnTy,
}

#[derive(Debug, Clone)]
struct LocalState {
    ty: OwnTy,
    moved: bool,
}

pub fn check_ownership(program: &HirProgram) -> Result<OwnershipReport, Vec<Diagnostic>> {
    let mut checker = OwnershipChecker::new(program);
    checker.check_program(program);
    checker.finish()
}

struct OwnershipChecker {
    functions: HashMap<String, FunctionSig>,
    structs: HashMap<String, HashMap<String, OwnTy>>,
    enums: HashSet<String>,
    enum_variants: HashSet<String>,
    diagnostics: Vec<Diagnostic>,
    checked_functions: usize,
}

impl OwnershipChecker {
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
                                    ty: OwnTy::Unknown,
                                })
                            })
                            .collect(),
                        ret: function
                            .return_type
                            .as_ref()
                            .map(own_ty_from_type)
                            .unwrap_or(OwnTy::Unit),
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
                                ty: OwnTy::Unknown,
                            })
                        })
                        .collect(),
                    ret: function
                        .return_type
                        .as_ref()
                        .map(own_ty_from_type)
                        .unwrap_or(OwnTy::Unit),
                },
            );
        }

        Self {
            functions,
            structs: program
                .structs
                .iter()
                .map(|item| {
                    (
                        item.name.clone(),
                        item.fields
                            .iter()
                            .map(|field| (field.name.clone(), own_ty_from_type(&field.ty_expr)))
                            .collect(),
                    )
                })
                .collect(),
            enums: program.enums.iter().map(|item| item.name.clone()).collect(),
            enum_variants: program
                .enums
                .iter()
                .flat_map(|item| item.variants.iter().map(|variant| variant.name.clone()))
                .collect(),
            diagnostics: Vec::new(),
            checked_functions: 0,
        }
    }

    fn check_program(&mut self, program: &HirProgram) {
        for function in &program.functions {
            self.check_function(function);
        }
        for test in &program.tests {
            let mut scope = HashMap::new();
            self.check_block(&test.body, &mut scope);
        }
    }

    fn check_function(&mut self, function: &HirFunction) {
        self.checked_functions += 1;
        let mut scope = HashMap::new();
        for param in &function.params {
            let ty = param
                .ty_expr
                .as_ref()
                .map(own_ty_from_type)
                .unwrap_or(OwnTy::Unknown);
            scope.insert(
                normalize_param_name(&param.name),
                LocalState { ty, moved: false },
            );
        }
        self.check_block(&function.body, &mut scope);
    }

    fn check_block(&mut self, block: &Block, scope: &mut HashMap<String, LocalState>) {
        for stmt in &block.statements {
            self.check_stmt(stmt, scope);
            if stmt_stops_control_flow(stmt) {
                break;
            }
        }
    }

    fn check_stmt(&mut self, stmt: &Stmt, scope: &mut HashMap<String, LocalState>) {
        match &stmt.data {
            StmtData::Let(stmt) => {
                let value_ty = stmt.value.as_ref().map(|value| {
                    let value_ty = self.infer_expr_ty(value, scope);
                    self.check_expr(value, scope, MoveMode::Move);
                    value_ty
                });
                let declared_ty = stmt.ty_expr.as_ref().map(own_ty_from_type);
                scope.insert(
                    stmt.name.clone(),
                    LocalState {
                        ty: declared_ty.or(value_ty).unwrap_or(OwnTy::Unknown),
                        moved: false,
                    },
                );
            }
            StmtData::Return(expr) | StmtData::Break(expr) => {
                if let Some(expr) = expr {
                    self.check_expr(expr, scope, MoveMode::Move);
                }
            }
            StmtData::Continue | StmtData::Raw => {}
            StmtData::Expr(expr) => self.check_expr(expr, scope, MoveMode::Read),
            StmtData::If(control) => {
                if let Some(condition) = &control.condition {
                    self.check_expr(condition, scope, MoveMode::Read);
                }
                self.check_child_block_and_propagate_moves(&control.body, scope);
            }
            StmtData::While(control) => {
                if let Some(condition) = &control.condition {
                    self.check_expr(condition, scope, MoveMode::Read);
                }
                self.check_child_block_and_propagate_moves(&control.body, scope);
            }
            StmtData::Match(expr) => self.check_match(expr, scope, MoveMode::Read),
            StmtData::For(stmt) => {
                self.check_expr(&stmt.iterator, scope, MoveMode::Read);
                let mut child = scope.clone();
                for binding in pattern_bindings(&stmt.pattern) {
                    child.insert(
                        binding,
                        LocalState {
                            ty: OwnTy::Unknown,
                            moved: false,
                        },
                    );
                }
                self.check_block(&stmt.body, &mut child);
                if block_may_fallthrough(&stmt.body) {
                    self.propagate_child_moves(scope, &child);
                }
            }
            StmtData::Loop(block) => {
                let mut child = scope.clone();
                self.check_block(block, &mut child);
                if block_can_break(block) {
                    self.propagate_child_moves(scope, &child);
                }
            }
            StmtData::Unsafe(block) => self.check_child_block_and_propagate_moves(block, scope),
        }
    }

    fn check_expr(&mut self, expr: &Expr, scope: &mut HashMap<String, LocalState>, mode: MoveMode) {
        match expr {
            Expr::Missing | Expr::Path(_) | Expr::Literal(_) | Expr::Raw(_) => {}
            Expr::Ident(name) => self.check_ident(name, scope, mode),
            Expr::Unary(expr) => match expr.op {
                UnaryOp::Ref | UnaryOp::MutRef => {
                    self.check_expr(&expr.expr, scope, MoveMode::Borrow)
                }
                UnaryOp::Deref | UnaryOp::Neg | UnaryOp::Not => {
                    self.check_expr(&expr.expr, scope, MoveMode::Read)
                }
            },
            Expr::Mut(expr) => self.check_expr(expr, scope, MoveMode::Borrow),
            Expr::Binary(expr) => {
                if expr.op.is_assignment() {
                    self.check_expr(&expr.left, scope, MoveMode::Read);
                    self.check_expr(&expr.right, scope, MoveMode::Move);
                } else {
                    self.check_expr(&expr.left, scope, MoveMode::Read);
                    self.check_expr(&expr.right, scope, MoveMode::Read);
                }
            }
            Expr::Index(expr) => {
                self.check_expr(&expr.target, scope, MoveMode::Read);
                self.check_expr(&expr.index, scope, MoveMode::Read);
            }
            Expr::Call(expr) => self.check_call(expr, scope),
            Expr::Member(expr) => self.check_member(expr, scope, mode),
            Expr::Await(expr) | Expr::Try(expr) => self.check_expr(expr, scope, MoveMode::Read),
            Expr::Struct(expr) => {
                for field in &expr.fields {
                    if let Some(value) = &field.value {
                        let mode = if matches!(value, Expr::Member(_)) {
                            MoveMode::Read
                        } else {
                            MoveMode::Move
                        };
                        self.check_expr(value, scope, mode);
                    }
                }
            }
            Expr::Object(expr) => {
                for field in &expr.fields {
                    self.check_expr(&field.value, scope, MoveMode::Read);
                }
            }
            Expr::Closure(expr) => {
                let captures = expr.is_move.then(|| closure_captures(expr, scope));
                let mut child = scope.clone();
                if let Some(captures) = captures {
                    for capture in captures {
                        self.check_ident(&capture, scope, MoveMode::Move);
                    }
                }
                for param in &expr.params {
                    let ty = param
                        .ty_expr
                        .as_ref()
                        .map(own_ty_from_type)
                        .unwrap_or(OwnTy::Unknown);
                    child.insert(
                        normalize_param_name(&param.name),
                        LocalState { ty, moved: false },
                    );
                }
                self.check_block(&expr.body, &mut child);
            }
            Expr::Match(expr) => self.check_match(expr, scope, mode),
            Expr::If(expr) => {
                self.check_expr(&expr.condition, scope, MoveMode::Read);
                let then_child = self.check_block_expr_scope(&expr.then_branch, scope, mode);
                let else_child = expr
                    .else_branch
                    .as_ref()
                    .map(|else_branch| self.check_block_expr_scope(else_branch, scope, mode));
                let then_child = block_exits_to_parent(&expr.then_branch).then_some(&then_child);
                let else_child = expr
                    .else_branch
                    .as_ref()
                    .is_some_and(block_exits_to_parent)
                    .then(|| else_child.as_ref())
                    .flatten();
                self.propagate_branch_moves(scope, then_child, else_child);
            }
            Expr::Block(block) => self.check_block_expr(block, scope, mode),
        }
    }

    fn check_child_block_and_propagate_moves(
        &mut self,
        block: &Block,
        scope: &mut HashMap<String, LocalState>,
    ) {
        let mut child = scope.clone();
        self.check_block(block, &mut child);
        if block_exits_to_parent(block) {
            self.propagate_child_moves(scope, &child);
        }
    }

    fn check_block_expr(
        &mut self,
        block: &Block,
        scope: &mut HashMap<String, LocalState>,
        mode: MoveMode,
    ) {
        let child = self.check_block_expr_scope(block, scope, mode);
        self.propagate_child_moves(scope, &child);
    }

    fn check_block_expr_scope(
        &mut self,
        block: &Block,
        scope: &HashMap<String, LocalState>,
        mode: MoveMode,
    ) -> HashMap<String, LocalState> {
        let mut child = scope.clone();
        let final_expr_index = block
            .statements
            .last()
            .filter(|stmt| matches!(stmt.data, StmtData::Expr(_)))
            .map(|_| block.statements.len() - 1);
        for (index, stmt) in block.statements.iter().enumerate() {
            if Some(index) == final_expr_index {
                if let StmtData::Expr(expr) = &stmt.data {
                    self.check_expr(expr, &mut child, mode);
                }
                break;
            }
            self.check_stmt(stmt, &mut child);
            if stmt_stops_control_flow(stmt) {
                break;
            }
        }
        child
    }

    fn propagate_child_moves(
        &self,
        scope: &mut HashMap<String, LocalState>,
        child: &HashMap<String, LocalState>,
    ) {
        for (name, local) in scope.iter_mut() {
            if let Some(child_local) = child.get(name) {
                local.moved |= child_local.moved;
            }
        }
    }

    fn propagate_branch_moves(
        &self,
        scope: &mut HashMap<String, LocalState>,
        then_child: Option<&HashMap<String, LocalState>>,
        else_child: Option<&HashMap<String, LocalState>>,
    ) {
        for (name, local) in scope.iter_mut() {
            let then_moved = then_child
                .and_then(|child| child.get(name))
                .is_some_and(|local| local.moved);
            let else_moved = else_child
                .and_then(|child| child.get(name))
                .is_some_and(|local| local.moved);
            local.moved |= then_moved || else_moved;
        }
    }

    fn check_member(
        &mut self,
        expr: &MemberExpr,
        scope: &mut HashMap<String, LocalState>,
        mode: MoveMode,
    ) {
        if mode == MoveMode::Move {
            if let Expr::Ident(target) = expr.target.as_ref() {
                if let Some(local) = scope.get(target) {
                    if local.moved {
                        self.error(format!("use of moved value `{target}`"));
                        return;
                    }
                    let field_ty = self.member_ty(&local.ty, &expr.member);
                    let Some(field_ty) = field_ty else {
                        self.check_expr(&expr.target, scope, MoveMode::Read);
                        return;
                    };
                    if is_copy_ty(&field_ty) {
                        self.check_expr(&expr.target, scope, MoveMode::Read);
                        return;
                    }
                    if !is_copy_ty(&field_ty) {
                        self.error(format!(
                            "cannot move field `{}` out of `{target}`; move-out from aggregate fields is not supported yet",
                            expr.member
                        ));
                        return;
                    }
                }
            }
        }
        self.check_expr(&expr.target, scope, MoveMode::Read);
    }

    fn member_ty(&self, target_ty: &OwnTy, field: &str) -> Option<OwnTy> {
        let OwnTy::Named(name) = target_ty else {
            return None;
        };
        self.structs
            .get(name)
            .and_then(|fields| fields.get(field))
            .cloned()
    }

    fn check_ident(&mut self, name: &str, scope: &mut HashMap<String, LocalState>, mode: MoveMode) {
        let Some(local) = scope.get_mut(name) else {
            return;
        };
        if local.moved {
            self.error(format!("use of moved value `{name}`"));
            return;
        }
        if mode == MoveMode::Move && !is_copy_ty(&local.ty) {
            local.moved = true;
        }
    }

    fn check_call(&mut self, expr: &CallExpr, scope: &mut HashMap<String, LocalState>) {
        if is_vec_push_call(&expr.callee) {
            self.check_expr(&expr.callee, scope, MoveMode::Read);
            if let Some(vec) = expr.args.first() {
                self.check_expr(vec, scope, MoveMode::Read);
            }
            if let Some(value) = expr.args.get(1) {
                self.check_expr(value, scope, MoveMode::Move);
            }
            for arg in expr.args.iter().skip(2) {
                self.check_expr(arg, scope, MoveMode::Read);
            }
            return;
        }
        if is_map_put_call(&expr.callee) {
            self.check_expr(&expr.callee, scope, MoveMode::Read);
            if is_alloc_map_put_call(&expr.callee) {
                for arg in expr.args.iter().take(2) {
                    self.check_expr(arg, scope, MoveMode::Read);
                }
                for value in expr.args.iter().skip(2).take(2) {
                    self.check_expr(value, scope, MoveMode::Move);
                }
                for arg in expr.args.iter().skip(4) {
                    self.check_expr(arg, scope, MoveMode::Read);
                }
            } else {
                if let Some(map) = expr.args.first() {
                    self.check_expr(map, scope, MoveMode::Read);
                }
                for value in expr.args.iter().skip(1).take(2) {
                    self.check_expr(value, scope, MoveMode::Move);
                }
                for arg in expr.args.iter().skip(3) {
                    self.check_expr(arg, scope, MoveMode::Read);
                }
            }
            return;
        }
        if is_box_consuming_call(&expr.callee) {
            self.check_expr(&expr.callee, scope, MoveMode::Read);
            if let Some(arg) = expr.args.first() {
                self.check_expr(arg, scope, MoveMode::Move);
            }
            for arg in expr.args.iter().skip(1) {
                self.check_expr(arg, scope, MoveMode::Read);
            }
            return;
        }
        if is_std_string_read_call(&expr.callee) {
            self.check_expr(&expr.callee, scope, MoveMode::Read);
            for arg in &expr.args {
                self.check_expr(arg, scope, MoveMode::Read);
            }
            return;
        }
        if direct_callee_name(&expr.callee).is_some_and(|name| self.enum_variants.contains(name)) {
            for arg in &expr.args {
                self.check_expr(arg, scope, MoveMode::Move);
            }
            return;
        }

        let sig = direct_callee_name(&expr.callee)
            .and_then(|name| self.functions.get(name))
            .cloned();

        if let Some(sig) = sig {
            self.check_call_args_with_params(&expr.args, &sig.params, scope);
            for arg in expr.args.iter().skip(sig.params.len()) {
                self.check_expr(arg, scope, MoveMode::Read);
            }
            return;
        }

        self.check_expr(&expr.callee, scope, MoveMode::Read);
        if let OwnTy::Function { params, .. } = self.infer_expr_ty(&expr.callee, scope) {
            self.check_call_args_with_params(&expr.args, &params, scope);
            for arg in expr.args.iter().skip(params.len()) {
                self.check_expr(arg, scope, MoveMode::Read);
            }
            return;
        }
        for arg in &expr.args {
            self.check_expr(arg, scope, MoveMode::Read);
        }
    }

    fn check_call_args_with_params(
        &mut self,
        args: &[Expr],
        params: &[ParamMode],
        scope: &mut HashMap<String, LocalState>,
    ) {
        for (arg, param) in args.iter().zip(params) {
            if param.by_ref {
                self.check_borrow_arg(arg, scope);
            } else {
                let mode = if is_copy_ty(&param.ty) {
                    MoveMode::Read
                } else {
                    MoveMode::Move
                };
                self.check_expr(arg, scope, mode);
            }
        }
    }

    fn check_borrow_arg(&mut self, arg: &Expr, scope: &mut HashMap<String, LocalState>) {
        match arg {
            Expr::Mut(expr) => self.check_expr(expr, scope, MoveMode::Borrow),
            Expr::Unary(expr) if matches!(expr.op, UnaryOp::Ref | UnaryOp::MutRef) => {
                self.check_expr(&expr.expr, scope, MoveMode::Borrow);
            }
            _ => self.check_expr(arg, scope, MoveMode::Borrow),
        }
    }

    fn check_match(
        &mut self,
        expr: &MatchExpr,
        scope: &mut HashMap<String, LocalState>,
        mode: MoveMode,
    ) {
        self.check_expr(&expr.value, scope, mode);
        let mut arm_scopes = Vec::new();
        for arm in &expr.arms {
            let mut child = scope.clone();
            for binding in pattern_bindings(&arm.pattern) {
                child.insert(
                    binding,
                    LocalState {
                        ty: OwnTy::Unknown,
                        moved: false,
                    },
                );
            }
            self.check_expr(&arm.body, &mut child, mode);
            if expr_exits_to_parent(&arm.body) {
                arm_scopes.push(child);
            }
        }
        for (name, local) in scope.iter_mut() {
            if arm_scopes
                .iter()
                .any(|child| child.get(name).is_some_and(|local| local.moved))
            {
                local.moved = true;
            }
        }
    }

    fn infer_expr_ty(&self, expr: &Expr, scope: &HashMap<String, LocalState>) -> OwnTy {
        match expr {
            Expr::Missing | Expr::Path(_) | Expr::Raw(_) => OwnTy::Unknown,
            Expr::Ident(name) => scope
                .get(name)
                .map(|local| local.ty.clone())
                .or_else(|| self.functions.get(name).map(|sig| sig.ret.clone()))
                .unwrap_or(OwnTy::Unknown),
            Expr::Literal(literal) => match literal {
                Literal::Int(_) => OwnTy::Int,
                Literal::Float(_) => OwnTy::Float,
                Literal::String(_) => OwnTy::String,
                Literal::Char(_) => OwnTy::Char,
                Literal::Bool(_) => OwnTy::Bool,
                Literal::Unit => OwnTy::Unit,
            },
            Expr::Unary(expr) => match expr.op {
                UnaryOp::Ref | UnaryOp::MutRef => OwnTy::Ref,
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
                | BinaryOp::BoolOr => OwnTy::Bool,
                BinaryOp::Assign
                | BinaryOp::AddAssign
                | BinaryOp::SubAssign
                | BinaryOp::MulAssign
                | BinaryOp::DivAssign
                | BinaryOp::RemAssign
                | BinaryOp::BitAndAssign
                | BinaryOp::BitOrAssign
                | BinaryOp::ShlAssign
                | BinaryOp::ShrAssign => OwnTy::Unit,
                BinaryOp::Add
                | BinaryOp::Sub
                | BinaryOp::Mul
                | BinaryOp::Div
                | BinaryOp::Rem
                | BinaryOp::BitAnd
                | BinaryOp::BitOr
                | BinaryOp::BitXor
                | BinaryOp::Shl
                | BinaryOp::Shr => {
                    let left = self.infer_expr_ty(&expr.left, scope);
                    let right = self.infer_expr_ty(&expr.right, scope);
                    if expr.op == BinaryOp::Add
                        && (matches!(left, OwnTy::String) || matches!(right, OwnTy::String))
                    {
                        OwnTy::String
                    } else if matches!(left, OwnTy::Float) || matches!(right, OwnTy::Float) {
                        OwnTy::Float
                    } else {
                        OwnTy::Int
                    }
                }
            },
            Expr::Call(expr) => direct_callee_name(&expr.callee)
                .and_then(|name| self.functions.get(name))
                .map(|sig| sig.ret.clone())
                .or_else(|| match self.infer_expr_ty(&expr.callee, scope) {
                    OwnTy::Function { ret, .. } => Some(*ret),
                    _ => None,
                })
                .unwrap_or(OwnTy::Unknown),
            Expr::Index(_) => OwnTy::Int,
            Expr::Member(expr) => {
                let target_ty = self.infer_expr_ty(&expr.target, scope);
                self.member_ty(&target_ty, &expr.member)
                    .unwrap_or(OwnTy::Unknown)
            }
            Expr::Struct(expr) => {
                if self.structs.contains_key(&expr.name) || self.enums.contains(&expr.name) {
                    OwnTy::Named(expr.name.clone())
                } else {
                    OwnTy::Unknown
                }
            }
            Expr::Object(_) => OwnTy::Unknown,
            Expr::Closure(_) => OwnTy::Unknown,
            Expr::If(expr) => self.infer_if_ty(expr, scope),
            Expr::Block(block) => self.infer_block_ty(block, scope),
            Expr::Match(_) => OwnTy::Unknown,
        }
    }

    fn infer_if_ty(&self, expr: &IfExpr, scope: &HashMap<String, LocalState>) -> OwnTy {
        let Some(else_branch) = &expr.else_branch else {
            return OwnTy::Unknown;
        };
        let then_ty = self.infer_block_ty(&expr.then_branch, scope);
        let else_ty = self.infer_block_ty(else_branch, scope);
        if then_ty == else_ty {
            then_ty
        } else {
            OwnTy::Unknown
        }
    }

    fn infer_block_ty(&self, block: &Block, scope: &HashMap<String, LocalState>) -> OwnTy {
        let mut child = scope.clone();
        for stmt in &block.statements {
            match &stmt.data {
                StmtData::Let(stmt) => {
                    let ty = stmt
                        .ty_expr
                        .as_ref()
                        .map(own_ty_from_type)
                        .or_else(|| {
                            stmt.value
                                .as_ref()
                                .map(|value| self.infer_expr_ty(value, &child))
                        })
                        .unwrap_or(OwnTy::Unknown);
                    child.insert(stmt.name.clone(), LocalState { ty, moved: false });
                }
                StmtData::Expr(expr) => return self.infer_expr_ty(expr, &child),
                _ => {}
            }
        }
        OwnTy::Unknown
    }

    fn error(&mut self, message: String) {
        self.diagnostics.push(Diagnostic {
            message,
            location: None,
            code: None,
        });
    }

    fn finish(self) -> Result<OwnershipReport, Vec<Diagnostic>> {
        if self.diagnostics.is_empty() {
            Ok(OwnershipReport {
                checked_functions: self.checked_functions,
            })
        } else {
            Err(self.diagnostics)
        }
    }
}

fn param_mode(expr: &TypeExpr) -> ParamMode {
    match expr {
        TypeExpr::Ref { inner, .. } => ParamMode {
            by_ref: true,
            ty: own_ty_from_type(inner),
        },
        TypeExpr::RawPtr { .. } => ParamMode {
            by_ref: false,
            ty: OwnTy::Int,
        },
        _ => ParamMode {
            by_ref: false,
            ty: own_ty_from_type(expr),
        },
    }
}

fn own_ty_from_type(expr: &TypeExpr) -> OwnTy {
    match expr {
        TypeExpr::Path(path) => path
            .first()
            .map(|name| match name.as_str() {
                "Bool" => OwnTy::Bool,
                "Int" | "Int8" | "Int16" | "Int32" | "Int64" | "UInt" | "UInt8" | "UInt16"
                | "UInt32" | "UInt64" => OwnTy::Int,
                "Float32" | "Float64" => OwnTy::Float,
                "Char" => OwnTy::Char,
                "String" | "Str" => OwnTy::String,
                other => OwnTy::Named(other.to_string()),
            })
            .unwrap_or(OwnTy::Unknown),
        TypeExpr::Tuple(items) if items.is_empty() => OwnTy::Unit,
        TypeExpr::Tuple(_) => OwnTy::Unknown,
        TypeExpr::Generic { base, .. } => own_ty_from_type(base),
        TypeExpr::Ref { .. } => OwnTy::Ref,
        TypeExpr::RawPtr { .. } => OwnTy::Int,
        TypeExpr::Fn {
            params,
            return_type,
            ..
        } => OwnTy::Function {
            params: params.iter().map(param_mode).collect(),
            ret: Box::new(own_ty_from_type(return_type)),
        },
        TypeExpr::Impl(_) => OwnTy::Unknown,
        TypeExpr::Mut(inner) => own_ty_from_type(inner),
        TypeExpr::Missing => OwnTy::Unknown,
    }
}

fn is_copy_ty(ty: &OwnTy) -> bool {
    matches!(
        ty,
        OwnTy::Unknown
            | OwnTy::Unit
            | OwnTy::Bool
            | OwnTy::Int
            | OwnTy::Float
            | OwnTy::Char
            | OwnTy::Ref
            | OwnTy::Function { .. }
    )
}

fn block_may_fallthrough(block: &Block) -> bool {
    !matches!(
        block.statements.last().map(|stmt| &stmt.data),
        Some(StmtData::Return(_) | StmtData::Break(_) | StmtData::Continue)
    )
}

fn block_exits_to_parent(block: &Block) -> bool {
    block_may_fallthrough(block) || block_can_break(block)
}

fn block_can_break(block: &Block) -> bool {
    block.statements.iter().any(stmt_can_break)
}

fn stmt_can_break(stmt: &Stmt) -> bool {
    match &stmt.data {
        StmtData::Break(_) => true,
        StmtData::If(control) | StmtData::While(control) => block_can_break(&control.body),
        StmtData::Match(expr) => expr.arms.iter().any(|arm| expr_can_break(&arm.body)),
        StmtData::For(stmt) => block_can_break(&stmt.body),
        StmtData::Loop(block) | StmtData::Unsafe(block) => block_can_break(block),
        StmtData::Let(_)
        | StmtData::Return(_)
        | StmtData::Continue
        | StmtData::Expr(_)
        | StmtData::Raw => false,
    }
}

fn expr_can_break(expr: &Expr) -> bool {
    match expr {
        Expr::Block(block) => block_can_break(block),
        Expr::If(expr) => {
            block_can_break(&expr.then_branch)
                || expr
                    .else_branch
                    .as_ref()
                    .is_some_and(|branch| block_can_break(branch))
        }
        Expr::Match(expr) => expr.arms.iter().any(|arm| expr_can_break(&arm.body)),
        _ => false,
    }
}

fn stmt_stops_control_flow(stmt: &Stmt) -> bool {
    matches!(
        stmt.data,
        StmtData::Return(_) | StmtData::Break(_) | StmtData::Continue
    )
}

fn expr_may_fallthrough(expr: &Expr) -> bool {
    match expr {
        Expr::Block(block) => block_may_fallthrough(block),
        _ => true,
    }
}

fn expr_exits_to_parent(expr: &Expr) -> bool {
    expr_may_fallthrough(expr) || expr_can_break(expr)
}

fn direct_callee_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Ident(name) => Some(name),
        _ => None,
    }
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

fn is_box_consuming_call(expr: &Expr) -> bool {
    direct_callee_name(expr).is_some_and(|name| {
        name.ends_with("box__new")
            || name.ends_with("box__take_int")
            || name.ends_with("box__take_string")
            || name.ends_with("box__destroy_int")
    }) || matches!(
        expr,
        Expr::Member(MemberExpr { target, member })
            if matches!(member.as_str(), "new" | "take" | "destroy")
                && matches!(target.as_ref(), Expr::Ident(name) if name == "box")
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

fn closure_captures(expr: &ClosureExpr, scope: &HashMap<String, LocalState>) -> Vec<String> {
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
    scope: &HashMap<String, LocalState>,
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
    scope: &HashMap<String, LocalState>,
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
    scope: &HashMap<String, LocalState>,
    locals: &mut Vec<String>,
    captures: &mut Vec<String>,
) {
    match expr {
        Expr::Ident(name) => {
            if scope.contains_key(name) && !locals.contains(name) {
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
    scope: &HashMap<String, LocalState>,
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
    scope: &HashMap<String, LocalState>,
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
