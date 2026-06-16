use std::collections::HashMap;

use crate::ast::*;
use crate::hir::{HirFunction, HirProgram};
use crate::semantics::Diagnostic;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadReport {
    pub checked_functions: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ThreadTy {
    Unknown,
    Borrow,
    RawPtr,
    Send,
}

#[derive(Debug, Clone)]
struct Scope {
    locals: HashMap<String, ThreadTy>,
}

pub fn check_threads(program: &HirProgram) -> Result<ThreadReport, Vec<Diagnostic>> {
    let mut checker = ThreadChecker {
        diagnostics: Vec::new(),
        checked_functions: 0,
    };
    checker.check_program(program);
    checker.finish()
}

struct ThreadChecker {
    diagnostics: Vec<Diagnostic>,
    checked_functions: usize,
}

impl ThreadChecker {
    fn check_program(&mut self, program: &HirProgram) {
        for function in &program.functions {
            self.check_function(function);
        }
        for test in &program.tests {
            self.check_block(
                &test.body,
                &mut Scope {
                    locals: HashMap::new(),
                },
            );
        }
    }

    fn check_function(&mut self, function: &HirFunction) {
        self.checked_functions += 1;
        let mut scope = Scope {
            locals: HashMap::new(),
        };
        for param in &function.params {
            scope.locals.insert(
                normalize_param_name(&param.name),
                param
                    .ty_expr
                    .as_ref()
                    .map(thread_ty_from_type)
                    .unwrap_or(ThreadTy::Unknown),
            );
        }
        self.check_block(&function.body, &mut scope);
    }

    fn check_block(&mut self, block: &Block, scope: &mut Scope) {
        for stmt in &block.statements {
            self.check_stmt(stmt, scope);
        }
    }

    fn check_stmt(&mut self, stmt: &Stmt, scope: &mut Scope) {
        match &stmt.data {
            StmtData::Let(stmt) => {
                if let Some(value) = &stmt.value {
                    self.check_expr(value, scope);
                    scope
                        .locals
                        .insert(stmt.name.clone(), self.infer_expr_ty(value, scope));
                } else {
                    scope.locals.insert(stmt.name.clone(), ThreadTy::Unknown);
                }
            }
            StmtData::Return(expr) | StmtData::Break(expr) => {
                if let Some(expr) = expr {
                    self.check_expr(expr, scope);
                }
            }
            StmtData::Continue | StmtData::Raw => {}
            StmtData::If(control) | StmtData::While(control) => {
                if let Some(condition) = &control.condition {
                    self.check_expr(condition, scope);
                }
                let mut child = scope.clone();
                self.check_block(&control.body, &mut child);
            }
            StmtData::Match(expr) => self.check_match(expr, scope),
            StmtData::For(stmt) => {
                self.check_expr(&stmt.iterator, scope);
                let mut child = scope.clone();
                for binding in pattern_bindings(&stmt.pattern) {
                    child.locals.insert(binding, ThreadTy::Unknown);
                }
                self.check_block(&stmt.body, &mut child);
            }
            StmtData::Loop(block) | StmtData::Unsafe(block) => {
                let mut child = scope.clone();
                self.check_block(block, &mut child);
            }
            StmtData::Expr(expr) => self.check_expr(expr, scope),
        }
    }

    fn check_expr(&mut self, expr: &Expr, scope: &mut Scope) {
        match expr {
            Expr::Call(expr) if is_thread_spawn(&expr.callee) => self.check_spawn(expr, scope),
            Expr::Call(expr) if is_channel_send(&expr.callee) => {
                self.check_channel_send(expr, scope)
            }
            Expr::Call(expr) => {
                self.check_expr(&expr.callee, scope);
                for arg in &expr.args {
                    self.check_expr(arg, scope);
                }
            }
            Expr::Unary(expr) => self.check_expr(&expr.expr, scope),
            Expr::Mut(expr) | Expr::Await(expr) | Expr::Try(expr) => self.check_expr(expr, scope),
            Expr::Binary(expr) => {
                self.check_expr(&expr.left, scope);
                self.check_expr(&expr.right, scope);
            }
            Expr::Index(expr) => {
                self.check_expr(&expr.target, scope);
                self.check_expr(&expr.index, scope);
            }
            Expr::Member(expr) => self.check_expr(&expr.target, scope),
            Expr::Struct(expr) => {
                for field in &expr.fields {
                    if let Some(value) = &field.value {
                        self.check_expr(value, scope);
                    }
                }
            }
            Expr::Object(expr) => {
                for field in &expr.fields {
                    self.check_expr(&field.value, scope);
                }
            }
            Expr::Closure(expr) => {
                let mut child = scope.clone();
                for param in &expr.params {
                    child.locals.insert(
                        normalize_param_name(&param.name),
                        param
                            .ty_expr
                            .as_ref()
                            .map(thread_ty_from_type)
                            .unwrap_or(ThreadTy::Unknown),
                    );
                }
                self.check_block(&expr.body, &mut child);
            }
            Expr::Match(expr) => self.check_match(expr, scope),
            Expr::If(expr) => {
                self.check_expr(&expr.condition, scope);
                let mut then_scope = scope.clone();
                self.check_block(&expr.then_branch, &mut then_scope);
                if let Some(else_branch) = &expr.else_branch {
                    let mut else_scope = scope.clone();
                    self.check_block(else_branch, &mut else_scope);
                }
            }
            Expr::Block(block) => {
                let mut child = scope.clone();
                self.check_block(block, &mut child);
            }
            Expr::Missing | Expr::Ident(_) | Expr::Path(_) | Expr::Literal(_) | Expr::Raw(_) => {}
        }
    }

    fn check_spawn(&mut self, expr: &CallExpr, scope: &mut Scope) {
        match expr.args.first() {
            Some(Expr::Ident(_)) => return,
            Some(Expr::Closure(closure)) => {
                if !closure.is_move {
                    self.error("thread.spawn requires a move closure".to_string());
                }
                for capture in closure_captures(closure, scope) {
                    match scope.locals.get(&capture) {
                        Some(ThreadTy::Borrow) => self.error(format!(
                            "capture `{capture}` is a borrowed reference and cannot be sent to a thread"
                        )),
                        Some(ThreadTy::RawPtr) => {
                            self.error(format!("capture `{capture}` is not Send"))
                        }
                        _ => {}
                    }
                }
                self.check_expr(&Expr::Closure(closure.clone()), scope);
            }
            _ => self.error("thread.spawn requires a move closure or named function".to_string()),
        }
    }

    fn check_channel_send(&mut self, expr: &CallExpr, scope: &mut Scope) {
        for arg in &expr.args {
            self.check_expr(arg, scope);
        }
        let Some(value) = expr.args.get(1) else {
            return;
        };
        match self.infer_expr_ty(value, scope) {
            ThreadTy::Borrow => {
                self.error("channel send value is a borrowed reference and is not Send".to_string())
            }
            ThreadTy::RawPtr => self.error("channel send value is not Send".to_string()),
            ThreadTy::Unknown | ThreadTy::Send => {}
        }
    }

    fn check_match(&mut self, expr: &MatchExpr, scope: &mut Scope) {
        self.check_expr(&expr.value, scope);
        for arm in &expr.arms {
            self.check_expr(&arm.body, scope);
        }
    }

    fn infer_expr_ty(&self, expr: &Expr, scope: &Scope) -> ThreadTy {
        match expr {
            Expr::Ident(name) => scope.locals.get(name).cloned().unwrap_or(ThreadTy::Unknown),
            Expr::Unary(expr) if matches!(expr.op, UnaryOp::Ref | UnaryOp::MutRef) => {
                ThreadTy::Borrow
            }
            Expr::Literal(_) | Expr::Struct(_) | Expr::Object(_) | Expr::Closure(_) => {
                ThreadTy::Send
            }
            _ => ThreadTy::Unknown,
        }
    }

    fn error(&mut self, message: String) {
        self.diagnostics.push(Diagnostic {
            message,
            location: None,
            code: None,
        });
    }

    fn finish(self) -> Result<ThreadReport, Vec<Diagnostic>> {
        if self.diagnostics.is_empty() {
            Ok(ThreadReport {
                checked_functions: self.checked_functions,
            })
        } else {
            Err(self.diagnostics)
        }
    }
}

fn is_thread_spawn(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Ident(name)
            if name == "spawn" || name == "thread__spawn" || name == "async__spawn"
    ) || matches!(
        expr,
        Expr::Member(member)
            if member.member == "spawn"
                && matches!(member.target.as_ref(), Expr::Ident(name) if name == "thread" || name == "async")
    )
}

fn is_channel_send(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Ident(name) if name == "send"
            || name.ends_with("__send")
            || name.ends_with("__send_int")
            || name.ends_with("__send_bool")
            || name.ends_with("__send_string")
            || name.ends_with("__send_function")
    ) || matches!(
        expr,
        Expr::Member(member)
            if member.member.starts_with("send")
                && matches!(member.target.as_ref(), Expr::Ident(name) if name == "channel")
    )
}

fn thread_ty_from_type(expr: &TypeExpr) -> ThreadTy {
    match expr {
        TypeExpr::RawPtr { .. } => ThreadTy::RawPtr,
        TypeExpr::Ref { .. } => ThreadTy::Borrow,
        TypeExpr::Path(_) | TypeExpr::Generic { .. } | TypeExpr::Tuple(_) | TypeExpr::Fn { .. } => {
            ThreadTy::Send
        }
        TypeExpr::Impl(_) | TypeExpr::Mut(_) | TypeExpr::Missing => ThreadTy::Unknown,
    }
}

fn closure_captures(expr: &ClosureExpr, scope: &Scope) -> Vec<String> {
    let params: Vec<_> = expr
        .params
        .iter()
        .map(|param| normalize_param_name(&param.name))
        .collect();
    let mut captures = Vec::new();
    collect_block_captures(&expr.body, scope, &params, &mut captures);
    captures.sort();
    captures.dedup();
    captures
}

fn collect_block_captures(
    block: &Block,
    scope: &Scope,
    params: &[String],
    captures: &mut Vec<String>,
) {
    let mut local_params = params.to_vec();
    for stmt in &block.statements {
        if let StmtData::Let(stmt) = &stmt.data {
            local_params.push(stmt.name.clone());
        }
        collect_stmt_captures(stmt, scope, &local_params, captures);
    }
}

fn collect_stmt_captures(
    stmt: &Stmt,
    scope: &Scope,
    params: &[String],
    captures: &mut Vec<String>,
) {
    match &stmt.data {
        StmtData::Let(stmt) => {
            if let Some(value) = &stmt.value {
                collect_expr_captures(value, scope, params, captures);
            }
        }
        StmtData::Return(expr) | StmtData::Break(expr) => {
            if let Some(expr) = expr {
                collect_expr_captures(expr, scope, params, captures);
            }
        }
        StmtData::Continue | StmtData::Raw => {}
        StmtData::If(control) | StmtData::While(control) => {
            if let Some(condition) = &control.condition {
                collect_expr_captures(condition, scope, params, captures);
            }
            collect_block_captures(&control.body, scope, params, captures);
        }
        StmtData::Match(expr) => collect_match_captures(expr, scope, params, captures),
        StmtData::For(stmt) => {
            collect_expr_captures(&stmt.iterator, scope, params, captures);
            collect_block_captures(&stmt.body, scope, params, captures);
        }
        StmtData::Loop(block) | StmtData::Unsafe(block) => {
            collect_block_captures(block, scope, params, captures);
        }
        StmtData::Expr(expr) => collect_expr_captures(expr, scope, params, captures),
    }
}

fn collect_expr_captures(
    expr: &Expr,
    scope: &Scope,
    params: &[String],
    captures: &mut Vec<String>,
) {
    match expr {
        Expr::Ident(name) => {
            if scope.locals.contains_key(name) && !params.contains(name) {
                captures.push(name.clone());
            }
        }
        Expr::Unary(expr) => collect_expr_captures(&expr.expr, scope, params, captures),
        Expr::Mut(expr) | Expr::Await(expr) | Expr::Try(expr) => {
            collect_expr_captures(expr, scope, params, captures);
        }
        Expr::Binary(expr) => {
            collect_expr_captures(&expr.left, scope, params, captures);
            collect_expr_captures(&expr.right, scope, params, captures);
        }
        Expr::Index(expr) => {
            collect_expr_captures(&expr.target, scope, params, captures);
            collect_expr_captures(&expr.index, scope, params, captures);
        }
        Expr::Call(expr) => {
            collect_expr_captures(&expr.callee, scope, params, captures);
            for arg in &expr.args {
                collect_expr_captures(arg, scope, params, captures);
            }
        }
        Expr::Member(expr) => collect_expr_captures(&expr.target, scope, params, captures),
        Expr::Struct(expr) => {
            for field in &expr.fields {
                if let Some(value) = &field.value {
                    collect_expr_captures(value, scope, params, captures);
                }
            }
        }
        Expr::Object(expr) => {
            for field in &expr.fields {
                collect_expr_captures(&field.value, scope, params, captures);
            }
        }
        Expr::Match(expr) => collect_match_captures(expr, scope, params, captures),
        Expr::If(expr) => {
            collect_expr_captures(&expr.condition, scope, params, captures);
            collect_block_captures(&expr.then_branch, scope, params, captures);
            if let Some(else_branch) = &expr.else_branch {
                collect_block_captures(else_branch, scope, params, captures);
            }
        }
        Expr::Block(block) => collect_block_captures(block, scope, params, captures),
        Expr::Closure(expr) => collect_nested_closure_captures(expr, scope, params, captures),
        Expr::Missing | Expr::Path(_) | Expr::Literal(_) | Expr::Raw(_) => {}
    }
}

fn collect_nested_closure_captures(
    expr: &ClosureExpr,
    scope: &Scope,
    params: &[String],
    captures: &mut Vec<String>,
) {
    let mut nested_params = params.to_vec();
    for param in &expr.params {
        nested_params.push(normalize_param_name(&param.name));
    }
    collect_block_captures(&expr.body, scope, &nested_params, captures);
}

fn collect_match_captures(
    expr: &MatchExpr,
    scope: &Scope,
    params: &[String],
    captures: &mut Vec<String>,
) {
    collect_expr_captures(&expr.value, scope, params, captures);
    for arm in &expr.arms {
        collect_expr_captures(&arm.body, scope, params, captures);
    }
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
