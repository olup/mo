use std::collections::{HashMap, HashSet};

use crate::ast::*;
use crate::hir::{HirItemKind, HirModuleId, HirProgram};
use crate::semantics::Diagnostic;
use crate::std::{core_types, prelude_values};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolveReport {
    pub resolved_value_refs: usize,
}

pub fn resolve_program(program: &HirProgram) -> Result<ResolveReport, Vec<Diagnostic>> {
    let mut resolver = Resolver::new(program);

    for function in &program.functions {
        resolver.resolve_function(function);
    }
    for test in &program.tests {
        resolver.resolve_block(&test.body, &mut Scope::default());
    }

    resolver.finish()
}

#[derive(Default, Clone)]
struct Scope {
    locals: HashSet<String>,
    type_params: HashSet<String>,
    allow_raw_intrinsics: bool,
}

struct Resolver {
    modules: HashMap<HirModuleId, Vec<String>>,
    top_values: HashSet<String>,
    top_types: HashSet<String>,
    resolved_value_refs: usize,
    diagnostics: Vec<Diagnostic>,
}

impl Resolver {
    fn new(program: &HirProgram) -> Self {
        let mut top_values = builtin_values();
        let mut top_types = builtin_types();

        for item in &program.items {
            match item.kind {
                HirItemKind::Function
                | HirItemKind::ExternFunction
                | HirItemKind::Const
                | HirItemKind::Static => {
                    top_values.insert(item.name.clone());
                }
                HirItemKind::Struct
                | HirItemKind::Enum
                | HirItemKind::Interface
                | HirItemKind::TypeAlias => {
                    top_types.insert(item.name.clone());
                    if matches!(item.kind, HirItemKind::Struct | HirItemKind::Enum) {
                        top_values.insert(item.name.clone());
                    }
                }
                HirItemKind::Impl | HirItemKind::Test => {}
            }
        }

        let modules = program
            .modules
            .iter()
            .map(|module| (module.id, module.path.clone()))
            .collect();

        Self {
            modules,
            top_values,
            top_types,
            resolved_value_refs: 0,
            diagnostics: Vec::new(),
        }
    }

    fn is_core_unsafe_wrapper(&self, module: HirModuleId, name: &str) -> bool {
        (self
            .modules
            .get(&module)
            .is_some_and(|path| module_path_ends_with(path, &["core", "unsafe"]))
            && is_core_unsafe_wrapper_name(name))
            || (name.starts_with("core__") && is_core_unsafe_wrapper_name(name))
    }

    fn resolve_function(&mut self, function: &crate::hir::HirFunction) {
        let mut scope = Scope {
            allow_raw_intrinsics: self.is_core_unsafe_wrapper(function.module, &function.name),
            ..Scope::default()
        };
        if let Some(generics) = &function.generics {
            for name in generic_param_names(generics) {
                scope.type_params.insert(name);
            }
        }
        for param in &function.params {
            scope.locals.insert(normalize_param_name(&param.name));
            if let Some(ty) = &param.ty_expr {
                self.resolve_type(ty, &scope);
            }
        }
        if let Some(ty) = &function.return_type {
            self.resolve_type(ty, &scope);
        }
        self.resolve_block(&function.body, &mut scope);
    }

    fn resolve_block(&mut self, block: &Block, scope: &mut Scope) {
        for stmt in &block.statements {
            self.resolve_stmt(stmt, scope);
        }
    }

    fn resolve_child_block(&mut self, block: &Block, scope: &Scope) {
        let mut child = scope.clone();
        self.resolve_block(block, &mut child);
    }

    fn resolve_stmt(&mut self, stmt: &Stmt, scope: &mut Scope) {
        match &stmt.data {
            StmtData::Let(stmt) => {
                if let Some(ty) = &stmt.ty_expr {
                    self.resolve_type(ty, scope);
                }
                if let Some(value) = &stmt.value {
                    self.resolve_expr(value, scope);
                }
                scope.locals.insert(stmt.name.clone());
            }
            StmtData::Return(Some(expr)) | StmtData::Break(Some(expr)) | StmtData::Expr(expr) => {
                self.resolve_expr(expr, scope)
            }
            StmtData::If(control) | StmtData::While(control) => {
                if let Some(condition) = &control.condition {
                    self.resolve_expr(condition, scope);
                }
                self.resolve_child_block(&control.body, scope);
            }
            StmtData::Match(expr) => self.resolve_match(expr, scope),
            StmtData::For(stmt) => {
                self.resolve_expr(&stmt.iterator, scope);
                let mut child = scope.clone();
                for binding in pattern_bindings(&stmt.pattern) {
                    child.locals.insert(binding);
                }
                self.resolve_block(&stmt.body, &mut child);
            }
            StmtData::Loop(block) | StmtData::Unsafe(block) => {
                self.resolve_child_block(block, scope)
            }
            StmtData::Return(None) | StmtData::Break(None) | StmtData::Continue | StmtData::Raw => {
            }
        }
    }

    fn resolve_expr(&mut self, expr: &Expr, scope: &mut Scope) {
        match expr {
            Expr::Ident(name) => self.resolve_value_name(name, scope),
            Expr::Path(path) => {
                if let Some(first) = path.first() {
                    self.resolve_value_name(first, scope);
                }
            }
            Expr::Unary(expr) => self.resolve_expr(&expr.expr, scope),
            Expr::Mut(expr) | Expr::Await(expr) | Expr::Try(expr) => self.resolve_expr(expr, scope),
            Expr::Binary(expr) => {
                self.resolve_expr(&expr.left, scope);
                self.resolve_expr(&expr.right, scope);
            }
            Expr::Index(expr) => {
                self.resolve_expr(&expr.target, scope);
                self.resolve_expr(&expr.index, scope);
            }
            Expr::Call(expr) => {
                self.resolve_expr(&expr.callee, scope);
                for arg in &expr.args {
                    self.resolve_expr(arg, scope);
                }
            }
            Expr::Member(expr) => {
                if let Expr::Ident(name) = expr.target.as_ref() {
                    if name
                        .chars()
                        .next()
                        .is_some_and(|ch| ch.is_ascii_uppercase())
                        && !self.top_values.contains(name)
                    {
                        self.error(format!("unknown namespace `{name}`"));
                        return;
                    }
                }
                self.resolve_expr(&expr.target, scope)
            }
            Expr::Struct(expr) => {
                self.resolve_type_name(&expr.name);
                for field in &expr.fields {
                    if let Some(value) = &field.value {
                        self.resolve_expr(value, scope);
                    }
                }
            }
            Expr::Object(expr) => {
                for field in &expr.fields {
                    self.resolve_expr(&field.value, scope);
                }
            }
            Expr::Closure(expr) => {
                let mut child = scope.clone();
                for param in &expr.params {
                    if let Some(ty) = &param.ty_expr {
                        self.resolve_type(ty, &child);
                    }
                    child.locals.insert(normalize_param_name(&param.name));
                }
                self.resolve_block(&expr.body, &mut child);
            }
            Expr::Match(expr) => self.resolve_match(expr, scope),
            Expr::If(expr) => {
                self.resolve_expr(&expr.condition, scope);
                self.resolve_child_block(&expr.then_branch, scope);
                if let Some(else_branch) = &expr.else_branch {
                    self.resolve_child_block(else_branch, scope);
                }
            }
            Expr::Block(block) => self.resolve_child_block(block, scope),
            Expr::Missing | Expr::Literal(_) | Expr::Raw(_) => {}
        }
    }

    fn resolve_match(&mut self, expr: &MatchExpr, scope: &mut Scope) {
        self.resolve_expr(&expr.value, scope);
        for arm in &expr.arms {
            let mut child = scope.clone();
            for binding in pattern_bindings(&arm.pattern) {
                child.locals.insert(binding);
            }
            self.resolve_expr(&arm.body, &mut child);
        }
    }

    fn resolve_type(&mut self, ty: &TypeExpr, scope: &Scope) {
        match ty {
            TypeExpr::Path(path) => {
                if let Some(name) = path.first() {
                    if !scope.type_params.contains(name) {
                        self.resolve_type_name(name);
                    }
                }
            }
            TypeExpr::Generic { base, args } => {
                self.resolve_type(base, scope);
                for arg in args {
                    self.resolve_type(arg, scope);
                }
            }
            TypeExpr::Tuple(items) => {
                for item in items {
                    self.resolve_type(item, scope);
                }
            }
            TypeExpr::Fn {
                params,
                return_type,
                ..
            } => {
                for param in params {
                    self.resolve_type(param, scope);
                }
                self.resolve_type(return_type, scope);
            }
            TypeExpr::Ref { inner, .. }
            | TypeExpr::RawPtr { inner, .. }
            | TypeExpr::Impl(inner)
            | TypeExpr::Mut(inner) => self.resolve_type(inner, scope),
            TypeExpr::Missing => {}
        }
    }

    fn resolve_value_name(&mut self, name: &str, scope: &Scope) {
        if !should_resolve_value_name(name) {
            return;
        }
        if scope.allow_raw_intrinsics && is_raw_intrinsic(name) {
            self.resolved_value_refs += 1;
            return;
        }
        if scope.locals.contains(name) || self.top_values.contains(name) {
            self.resolved_value_refs += 1;
        } else {
            self.error(format!("unknown value `{name}`"));
        }
    }

    fn resolve_type_name(&mut self, name: &str) {
        if !should_resolve_type_name(name) {
            return;
        }
        if !self.top_types.contains(name) {
            self.error(format!("unknown type `{name}`"));
        }
    }

    fn error(&mut self, message: String) {
        self.diagnostics.push(Diagnostic {
            message,
            location: None,
            code: None,
        });
    }

    fn finish(self) -> Result<ResolveReport, Vec<Diagnostic>> {
        if self.diagnostics.is_empty() {
            Ok(ResolveReport {
                resolved_value_refs: self.resolved_value_refs,
            })
        } else {
            Err(self.diagnostics)
        }
    }
}

fn module_path_ends_with(path: &[String], suffix: &[&str]) -> bool {
    path.len() >= suffix.len()
        && path[path.len() - suffix.len()..]
            .iter()
            .map(String::as_str)
            .eq(suffix.iter().copied())
}

fn is_raw_intrinsic(name: &str) -> bool {
    matches!(
        name,
        "raw_alloc"
            | "raw_alloc_string"
            | "raw_free"
            | "raw_load8"
            | "raw_load64"
            | "raw_store8"
            | "raw_store64"
            | "raw_string_store8"
            | "raw_string_ptr"
            | "raw_string_clone_ptr"
            | "raw_string_from_ptr"
            | "raw_function_ptr"
            | "raw_function_from_ptr"
            | "raw_function_from_ptr_int"
            | "raw_function_from_ptr_handler"
            | "raw_function_from_ptr_request_handler"
            | "raw_function_from_ptr_response_handler"
            | "raw_write"
            | "raw_strlen"
            | "raw_string_concat"
            | "raw_int_to_string"
            | "raw_float_to_int"
            | "raw_set_nonblocking"
            | "raw_thread_spawn"
            | "raw_thread_join"
            | "raw_mem_alloc_count"
            | "raw_mem_free_count"
            | "raw_mem_live_bytes"
            | "raw_mem_high_water_bytes"
    )
}

fn is_core_unsafe_wrapper_name(name: &str) -> bool {
    matches!(
        name,
        "alloc"
            | "alloc_string"
            | "free"
            | "alloc_ptr"
            | "free_ptr"
            | "load8"
            | "load8_ptr"
            | "load64"
            | "string_store8"
            | "string_load8"
            | "string_ptr"
            | "string_clone_ptr"
            | "string_from_ptr"
            | "function_ptr"
            | "function_from_ptr"
            | "function_ptr_int"
            | "function_from_ptr_int"
            | "function_ptr_handler"
            | "function_from_ptr_handler"
            | "function_ptr_request_handler"
            | "function_from_ptr_request_handler"
            | "function_ptr_response_handler"
            | "function_from_ptr_response_handler"
            | "store8"
            | "store8_ptr"
            | "store64"
            | "store32le"
            | "set_nonblocking_fd"
            | "thread_spawn"
            | "thread_join"
            | "write"
            | "strlen"
            | "string_concat"
            | "int_to_string"
            | "float_to_int"
            | "mem_alloc_count"
            | "mem_free_count"
            | "mem_live_bytes"
            | "mem_high_water_bytes"
    ) || name.ends_with("__alloc")
        || name.ends_with("__alloc_string")
        || name.ends_with("__free")
        || name.ends_with("__alloc_ptr")
        || name.ends_with("__free_ptr")
        || name.ends_with("__load8")
        || name.ends_with("__load8_ptr")
        || name.ends_with("__load64")
        || name.ends_with("__string_store8")
        || name.ends_with("__string_load8")
        || name.ends_with("__string_ptr")
        || name.ends_with("__string_clone_ptr")
        || name.ends_with("__string_from_ptr")
        || name.ends_with("__function_ptr")
        || name.ends_with("__function_from_ptr")
        || name.ends_with("__function_ptr_int")
        || name.ends_with("__function_from_ptr_int")
        || name.ends_with("__function_ptr_handler")
        || name.ends_with("__function_from_ptr_handler")
        || name.ends_with("__function_ptr_request_handler")
        || name.ends_with("__function_from_ptr_request_handler")
        || name.ends_with("__function_ptr_response_handler")
        || name.ends_with("__function_from_ptr_response_handler")
        || name.ends_with("__store8")
        || name.ends_with("__store8_ptr")
        || name.ends_with("__store64")
        || name.ends_with("__store32le")
        || name.ends_with("__set_nonblocking_fd")
        || name.ends_with("__thread_spawn")
        || name.ends_with("__thread_join")
        || name.ends_with("__write")
        || name.ends_with("__strlen")
        || name.ends_with("__string_concat")
        || name.ends_with("__int_to_string")
        || name.ends_with("__float_to_int")
        || name.ends_with("__mem_alloc_count")
        || name.ends_with("__mem_free_count")
        || name.ends_with("__mem_live_bytes")
        || name.ends_with("__mem_high_water_bytes")
}

fn should_resolve_value_name(name: &str) -> bool {
    (name
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_lowercase() || ch == '_')
        || name.contains("__"))
        && !matches!(name, "_" | "self")
}

fn should_resolve_type_name(name: &str) -> bool {
    name.chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_uppercase())
}

fn normalize_param_name(name: &str) -> String {
    match name {
        "self" | "&self" | "&mut self" => "self".to_string(),
        _ => name.to_string(),
    }
}

fn generic_param_names(generics: &str) -> Vec<String> {
    generics
        .split(',')
        .filter_map(|part| {
            let name = part
                .trim()
                .split(|ch: char| ch == ':' || ch.is_whitespace())
                .next()
                .unwrap_or("")
                .trim();
            if name.is_empty() {
                None
            } else {
                Some(name.to_string())
            }
        })
        .collect()
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

fn builtin_values() -> HashSet<String> {
    prelude_values().map(str::to_string).collect()
}

fn builtin_types() -> HashSet<String> {
    core_types().map(str::to_string).collect()
}
