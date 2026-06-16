use std::collections::{HashMap, HashSet};

use crate::ast::*;
use crate::hir::{HirFunction, HirProgram};
use crate::semantics::Diagnostic;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BorrowReport {
    pub checked_functions: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AccessMode {
    Read,
    Move,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BorrowKind {
    Shared,
    Mutable,
}

#[derive(Debug, Clone)]
struct ActiveBorrow {
    borrower: String,
    kind: BorrowKind,
}

#[derive(Debug, Clone, Default)]
struct BorrowScope {
    locals: HashSet<String>,
    borrows: HashMap<String, Vec<ActiveBorrow>>,
}

#[derive(Debug, Clone)]
struct FunctionSig {
    params: Vec<ParamMode>,
}

#[derive(Debug, Clone, Copy)]
struct ParamMode {
    by_ref: bool,
    mutable: bool,
}

pub fn check_borrows(program: &HirProgram) -> Result<BorrowReport, Vec<Diagnostic>> {
    let mut checker = BorrowChecker::new(program);
    checker.check_program(program);
    checker.finish()
}

struct BorrowChecker {
    functions: HashMap<String, FunctionSig>,
    diagnostics: Vec<Diagnostic>,
    checked_functions: usize,
}

impl BorrowChecker {
    fn new(program: &HirProgram) -> Self {
        let functions = program
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
                                    mutable: false,
                                })
                            })
                            .collect(),
                    },
                )
            })
            .collect();

        Self {
            functions,
            diagnostics: Vec::new(),
            checked_functions: 0,
        }
    }

    fn check_program(&mut self, program: &HirProgram) {
        for function in &program.functions {
            self.check_function(function);
        }
        for test in &program.tests {
            let mut scope = BorrowScope::default();
            self.check_block(&test.body, &mut scope);
        }
    }

    fn check_function(&mut self, function: &HirFunction) {
        self.checked_functions += 1;
        let mut scope = BorrowScope::default();
        for param in &function.params {
            scope.locals.insert(normalize_param_name(&param.name));
        }
        self.check_block(&function.body, &mut scope);
    }

    fn check_block(&mut self, block: &Block, scope: &mut BorrowScope) {
        let last_uses = last_uses(block);
        for (index, stmt) in block.statements.iter().enumerate() {
            self.check_stmt(stmt, scope);
            release_finished_borrows(scope, &last_uses, index);
        }
    }

    fn check_stmt(&mut self, stmt: &Stmt, scope: &mut BorrowScope) {
        match &stmt.data {
            StmtData::Let(stmt) => {
                if let Some(value) = &stmt.value {
                    if let Some((owner, kind)) = borrowed_ident(value) {
                        self.start_borrow(owner, stmt.name.clone(), kind, scope);
                        scope.locals.insert(stmt.name.clone());
                    } else {
                        self.check_expr(value, scope, AccessMode::Move);
                        scope.locals.insert(stmt.name.clone());
                    }
                } else {
                    scope.locals.insert(stmt.name.clone());
                }
            }
            StmtData::Return(expr) => {
                if let Some(expr) = expr {
                    self.check_return(expr, scope);
                }
            }
            StmtData::Break(expr) => {
                if let Some(expr) = expr {
                    self.check_expr(expr, scope, AccessMode::Move);
                }
            }
            StmtData::Continue | StmtData::Raw => {}
            StmtData::Expr(expr) => self.check_expr(expr, scope, AccessMode::Read),
            StmtData::If(control) | StmtData::While(control) => {
                if let Some(condition) = &control.condition {
                    self.check_expr(condition, scope, AccessMode::Read);
                }
                let mut child = scope.clone();
                self.check_block(&control.body, &mut child);
            }
            StmtData::Match(expr) => self.check_match(expr, scope),
            StmtData::For(stmt) => {
                self.check_expr(&stmt.iterator, scope, AccessMode::Read);
                let mut child = scope.clone();
                for binding in pattern_bindings(&stmt.pattern) {
                    child.locals.insert(binding);
                }
                self.check_block(&stmt.body, &mut child);
            }
            StmtData::Loop(block) | StmtData::Unsafe(block) => {
                let mut child = scope.clone();
                self.check_block(block, &mut child);
            }
        }
    }

    fn check_expr(&mut self, expr: &Expr, scope: &mut BorrowScope, mode: AccessMode) {
        match expr {
            Expr::Missing | Expr::Path(_) | Expr::Literal(_) | Expr::Raw(_) => {}
            Expr::Ident(name) => self.check_ident(name, scope, mode),
            Expr::Unary(expr) => match expr.op {
                UnaryOp::Ref => self.check_temporary_borrow(&expr.expr, BorrowKind::Shared, scope),
                UnaryOp::MutRef => {
                    self.check_temporary_borrow(&expr.expr, BorrowKind::Mutable, scope)
                }
                UnaryOp::Deref | UnaryOp::Neg | UnaryOp::Not => {
                    self.check_expr(&expr.expr, scope, AccessMode::Read);
                }
            },
            Expr::Mut(expr) => self.check_temporary_borrow(expr, BorrowKind::Mutable, scope),
            Expr::Binary(expr) => {
                if expr.op.is_assignment() {
                    self.check_expr(&expr.left, scope, AccessMode::Read);
                    self.check_expr(&expr.right, scope, AccessMode::Move);
                } else {
                    self.check_expr(&expr.left, scope, AccessMode::Read);
                    self.check_expr(&expr.right, scope, AccessMode::Read);
                }
            }
            Expr::Index(expr) => {
                self.check_expr(&expr.target, scope, AccessMode::Read);
                self.check_expr(&expr.index, scope, AccessMode::Read);
            }
            Expr::Call(expr) => self.check_call(expr, scope),
            Expr::Member(expr) => self.check_expr(&expr.target, scope, AccessMode::Read),
            Expr::Await(expr) | Expr::Try(expr) => self.check_expr(expr, scope, AccessMode::Read),
            Expr::Struct(expr) => {
                for field in &expr.fields {
                    if let Some(value) = &field.value {
                        self.check_expr(value, scope, AccessMode::Move);
                    }
                }
            }
            Expr::Object(expr) => {
                for field in &expr.fields {
                    self.check_expr(&field.value, scope, AccessMode::Read);
                }
            }
            Expr::Closure(expr) => {
                let mut child = scope.clone();
                for param in &expr.params {
                    child.locals.insert(normalize_param_name(&param.name));
                }
                self.check_block(&expr.body, &mut child);
            }
            Expr::Match(expr) => self.check_match(expr, scope),
            Expr::If(expr) => {
                self.check_expr(&expr.condition, scope, AccessMode::Read);
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
        }
    }

    fn check_call(&mut self, expr: &CallExpr, scope: &mut BorrowScope) {
        let sig = direct_callee_name(&expr.callee)
            .and_then(|name| self.functions.get(name))
            .cloned();

        if let Some(sig) = sig {
            for (arg, param) in expr.args.iter().zip(&sig.params) {
                if param.by_ref {
                    let kind = if param.mutable {
                        BorrowKind::Mutable
                    } else {
                        BorrowKind::Shared
                    };
                    self.check_temporary_borrow(call_borrow_target(arg), kind, scope);
                } else {
                    self.check_expr(arg, scope, AccessMode::Move);
                }
            }
            for arg in expr.args.iter().skip(sig.params.len()) {
                self.check_expr(arg, scope, AccessMode::Read);
            }
            return;
        }

        self.check_expr(&expr.callee, scope, AccessMode::Read);
        for arg in &expr.args {
            self.check_expr(arg, scope, AccessMode::Read);
        }
    }

    fn check_match(&mut self, expr: &MatchExpr, scope: &mut BorrowScope) {
        self.check_expr(&expr.value, scope, AccessMode::Read);
        for arm in &expr.arms {
            let mut child = scope.clone();
            for binding in pattern_bindings(&arm.pattern) {
                child.locals.insert(binding);
            }
            self.check_expr(&arm.body, &mut child, AccessMode::Read);
        }
    }

    fn check_return(&mut self, expr: &Expr, scope: &mut BorrowScope) {
        if let Some(name) = returned_local_ref(expr) {
            if scope.locals.contains(name) {
                self.error(format!("cannot return reference to local `{name}`"));
            }
        }
        self.check_expr(expr, scope, AccessMode::Move);
    }

    fn check_ident(&mut self, name: &str, scope: &BorrowScope, mode: AccessMode) {
        let Some(active) = scope.borrows.get(name).filter(|items| !items.is_empty()) else {
            return;
        };
        match mode {
            AccessMode::Read => {
                if active
                    .iter()
                    .any(|borrow| borrow.kind == BorrowKind::Mutable)
                {
                    self.error(format!("cannot use `{name}` while mutably borrowed"));
                }
            }
            AccessMode::Move => {
                self.error(format!("cannot move `{name}` while borrowed"));
            }
        }
    }

    fn check_temporary_borrow(&mut self, expr: &Expr, kind: BorrowKind, scope: &mut BorrowScope) {
        if let Expr::Ident(name) = expr {
            self.ensure_can_borrow(name, kind, scope);
        } else {
            self.check_expr(expr, scope, AccessMode::Read);
        }
    }

    fn start_borrow(
        &mut self,
        owner: &str,
        borrower: String,
        kind: BorrowKind,
        scope: &mut BorrowScope,
    ) {
        self.ensure_can_borrow(owner, kind, scope);
        scope
            .borrows
            .entry(owner.to_string())
            .or_default()
            .push(ActiveBorrow { borrower, kind });
    }

    fn ensure_can_borrow(&mut self, owner: &str, kind: BorrowKind, scope: &BorrowScope) {
        let Some(active) = scope.borrows.get(owner).filter(|items| !items.is_empty()) else {
            return;
        };
        match kind {
            BorrowKind::Shared => {
                if active
                    .iter()
                    .any(|borrow| borrow.kind == BorrowKind::Mutable)
                {
                    self.error(format!("cannot borrow `{owner}` while mutably borrowed"));
                }
            }
            BorrowKind::Mutable => {
                self.error(format!("cannot mutably borrow `{owner}` while borrowed"));
            }
        }
    }

    fn error(&mut self, message: String) {
        self.diagnostics.push(Diagnostic {
            message,
            location: None,
            code: None,
        });
    }

    fn finish(self) -> Result<BorrowReport, Vec<Diagnostic>> {
        if self.diagnostics.is_empty() {
            Ok(BorrowReport {
                checked_functions: self.checked_functions,
            })
        } else {
            Err(self.diagnostics)
        }
    }
}

fn param_mode(expr: &TypeExpr) -> ParamMode {
    match expr {
        TypeExpr::Ref { mutable, .. } => ParamMode {
            by_ref: true,
            mutable: *mutable,
        },
        TypeExpr::RawPtr { .. } => ParamMode {
            by_ref: false,
            mutable: false,
        },
        _ => ParamMode {
            by_ref: false,
            mutable: false,
        },
    }
}

fn borrowed_ident(expr: &Expr) -> Option<(&str, BorrowKind)> {
    match expr {
        Expr::Unary(expr) if expr.op == UnaryOp::Ref => {
            direct_ident(&expr.expr).map(|name| (name, BorrowKind::Shared))
        }
        Expr::Unary(expr) if expr.op == UnaryOp::MutRef => {
            direct_ident(&expr.expr).map(|name| (name, BorrowKind::Mutable))
        }
        Expr::Mut(expr) => direct_ident(expr).map(|name| (name, BorrowKind::Mutable)),
        _ => None,
    }
}

fn returned_local_ref(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Unary(expr) if matches!(expr.op, UnaryOp::Ref | UnaryOp::MutRef) => {
            direct_ident(&expr.expr)
        }
        Expr::Mut(expr) => direct_ident(expr),
        _ => None,
    }
}

fn call_borrow_target(arg: &Expr) -> &Expr {
    match arg {
        Expr::Mut(expr) => expr,
        Expr::Unary(expr) if matches!(expr.op, UnaryOp::Ref | UnaryOp::MutRef) => &expr.expr,
        _ => arg,
    }
}

fn direct_ident(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Ident(name) => Some(name),
        _ => None,
    }
}

fn direct_callee_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Ident(name) => Some(name),
        _ => None,
    }
}

fn release_finished_borrows(
    scope: &mut BorrowScope,
    last_uses: &HashMap<String, usize>,
    current: usize,
) {
    let mut empty_owners = Vec::new();
    for (owner, borrows) in &mut scope.borrows {
        borrows.retain(|borrow| {
            last_uses
                .get(&borrow.borrower)
                .is_some_and(|last_use| *last_use > current)
        });
        if borrows.is_empty() {
            empty_owners.push(owner.clone());
        }
    }
    for owner in empty_owners {
        scope.borrows.remove(&owner);
    }
}

fn last_uses(block: &Block) -> HashMap<String, usize> {
    let mut uses = HashMap::new();
    for (index, stmt) in block.statements.iter().enumerate() {
        collect_stmt_idents(stmt, index, &mut uses);
    }
    uses
}

fn collect_stmt_idents(stmt: &Stmt, index: usize, uses: &mut HashMap<String, usize>) {
    match &stmt.data {
        StmtData::Let(stmt) => {
            if let Some(value) = &stmt.value {
                collect_expr_idents(value, index, uses);
            }
        }
        StmtData::Return(expr) | StmtData::Break(expr) => {
            if let Some(expr) = expr {
                collect_expr_idents(expr, index, uses);
            }
        }
        StmtData::Continue | StmtData::Raw => {}
        StmtData::Expr(expr) => collect_expr_idents(expr, index, uses),
        StmtData::If(control) | StmtData::While(control) => {
            if let Some(condition) = &control.condition {
                collect_expr_idents(condition, index, uses);
            }
            for stmt in &control.body.statements {
                collect_stmt_idents(stmt, index, uses);
            }
        }
        StmtData::Match(expr) => collect_match_idents(expr, index, uses),
        StmtData::For(stmt) => {
            collect_expr_idents(&stmt.iterator, index, uses);
            for stmt in &stmt.body.statements {
                collect_stmt_idents(stmt, index, uses);
            }
        }
        StmtData::Loop(block) | StmtData::Unsafe(block) => {
            for stmt in &block.statements {
                collect_stmt_idents(stmt, index, uses);
            }
        }
    }
}

fn collect_expr_idents(expr: &Expr, index: usize, uses: &mut HashMap<String, usize>) {
    match expr {
        Expr::Missing | Expr::Path(_) | Expr::Literal(_) | Expr::Raw(_) => {}
        Expr::Ident(name) => {
            uses.insert(name.clone(), index);
        }
        Expr::Unary(expr) => collect_expr_idents(&expr.expr, index, uses),
        Expr::Mut(expr) | Expr::Await(expr) | Expr::Try(expr) => {
            collect_expr_idents(expr, index, uses);
        }
        Expr::Binary(expr) => {
            collect_expr_idents(&expr.left, index, uses);
            collect_expr_idents(&expr.right, index, uses);
        }
        Expr::Index(expr) => {
            collect_expr_idents(&expr.target, index, uses);
            collect_expr_idents(&expr.index, index, uses);
        }
        Expr::Call(expr) => {
            collect_expr_idents(&expr.callee, index, uses);
            for arg in &expr.args {
                collect_expr_idents(arg, index, uses);
            }
        }
        Expr::Member(expr) => collect_expr_idents(&expr.target, index, uses),
        Expr::Struct(expr) => {
            for field in &expr.fields {
                if let Some(value) = &field.value {
                    collect_expr_idents(value, index, uses);
                }
            }
        }
        Expr::Object(expr) => {
            for field in &expr.fields {
                collect_expr_idents(&field.value, index, uses);
            }
        }
        Expr::Closure(expr) => {
            for stmt in &expr.body.statements {
                collect_stmt_idents(stmt, index, uses);
            }
        }
        Expr::Match(expr) => collect_match_idents(expr, index, uses),
        Expr::If(expr) => {
            collect_expr_idents(&expr.condition, index, uses);
            for stmt in &expr.then_branch.statements {
                collect_stmt_idents(stmt, index, uses);
            }
            if let Some(else_branch) = &expr.else_branch {
                for stmt in &else_branch.statements {
                    collect_stmt_idents(stmt, index, uses);
                }
            }
        }
        Expr::Block(block) => {
            for stmt in &block.statements {
                collect_stmt_idents(stmt, index, uses);
            }
        }
    }
}

fn collect_match_idents(expr: &MatchExpr, index: usize, uses: &mut HashMap<String, usize>) {
    collect_expr_idents(&expr.value, index, uses);
    for arm in &expr.arms {
        collect_expr_idents(&arm.body, index, uses);
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
