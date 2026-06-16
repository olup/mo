use std::collections::{BTreeSet, HashMap, HashSet};

use crate::ast::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    triple: String,
    symbols: HashSet<String>,
}

impl Target {
    pub fn macos_aarch64() -> Self {
        Self::new(
            "aarch64-apple-darwin",
            ["macos", "aarch64", "apple", "darwin", "ptr64"],
        )
    }

    pub fn macos_x86_64() -> Self {
        Self::new(
            "x86_64-apple-darwin",
            ["macos", "x86_64", "apple", "darwin", "ptr64"],
        )
    }

    pub fn linux_x86_64() -> Self {
        Self::new(
            "x86_64-unknown-linux-gnu",
            ["linux", "x86_64", "unknown", "gnu", "ptr64"],
        )
    }

    pub fn linux_aarch64() -> Self {
        Self::new(
            "aarch64-unknown-linux-gnu",
            ["linux", "aarch64", "unknown", "gnu", "ptr64"],
        )
    }

    pub fn host() -> Self {
        if cfg!(target_os = "macos") && cfg!(target_arch = "aarch64") {
            Self::macos_aarch64()
        } else if cfg!(target_os = "macos") && cfg!(target_arch = "x86_64") {
            Self::macos_x86_64()
        } else if cfg!(target_os = "linux") && cfg!(target_arch = "x86_64") {
            Self::linux_x86_64()
        } else if cfg!(target_os = "linux") && cfg!(target_arch = "aarch64") {
            Self::linux_aarch64()
        } else {
            Self::new("unknown", ["unknown"])
        }
    }

    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "native" | "host" => Ok(Self::host()),
            "macos-aarch64" | "aarch64-macos" | "aarch64-apple-darwin" => Ok(Self::macos_aarch64()),
            "macos-x86_64" | "x86_64-macos" | "x86_64-apple-darwin" => Ok(Self::macos_x86_64()),
            "linux-x86_64" | "x86_64-linux" | "x86_64-unknown-linux-gnu" => {
                Ok(Self::linux_x86_64())
            }
            "linux-aarch64" | "aarch64-linux" | "aarch64-unknown-linux-gnu" => {
                Ok(Self::linux_aarch64())
            }
            other => Err(format!("unsupported target `{other}`")),
        }
    }

    pub fn new<const N: usize>(triple: &str, symbols: [&str; N]) -> Self {
        Self {
            triple: triple.to_string(),
            symbols: symbols.into_iter().map(str::to_string).collect(),
        }
    }

    pub fn has(&self, symbol: &str) -> bool {
        self.symbols.contains(symbol.trim_start_matches('.'))
    }

    pub fn triple(&self) -> &str {
        &self.triple
    }

    pub fn is_host(&self) -> bool {
        self.triple == Self::host().triple
    }

    pub fn object_format(&self) -> ObjectFormat {
        if self.has("darwin") {
            ObjectFormat::MachO
        } else if self.has("linux") {
            ObjectFormat::Elf
        } else {
            ObjectFormat::Unknown
        }
    }

    pub fn default_linker(&self) -> &'static str {
        "cc"
    }

    pub fn zig_triple(&self) -> String {
        if self.has("linux") && self.has("x86_64") {
            "x86_64-linux-gnu".to_string()
        } else if self.has("linux") && self.has("aarch64") {
            "aarch64-linux-gnu".to_string()
        } else if self.has("darwin") && self.has("aarch64") {
            "aarch64-macos".to_string()
        } else if self.has("darwin") && self.has("x86_64") {
            "x86_64-macos".to_string()
        } else {
            self.triple.clone()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectFormat {
    MachO,
    Elf,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub message: String,
    pub location: Option<String>,
    pub code: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckReport {
    pub enabled_symbols: Vec<String>,
    pub test_count: usize,
}

pub fn check_program(program: &Program, target: &Target) -> Result<CheckReport, Vec<Diagnostic>> {
    let mut checker = Checker::new(target);
    checker.check_items(&program.items);
    checker.finish()
}

struct Checker<'a> {
    target: &'a Target,
    top_level: HashMap<String, ()>,
    enabled_symbols: BTreeSet<String>,
    test_count: usize,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> Checker<'a> {
    fn new(target: &'a Target) -> Self {
        Self {
            target,
            top_level: HashMap::new(),
            enabled_symbols: BTreeSet::new(),
            test_count: 0,
            diagnostics: Vec::new(),
        }
    }

    fn check_items(&mut self, items: &[Item]) {
        for item in items {
            self.check_item(item);
        }
    }

    fn check_item(&mut self, item: &Item) {
        match item {
            Item::Directive(directive) => {
                if directive.name == "target" {
                    if target_predicate_matches(&directive.args, self.target) {
                        self.check_items(&directive.items);
                    }
                } else {
                    self.check_items(&directive.items);
                }
            }
            Item::Struct(item) => self.define_top_level(&item.name, None),
            Item::Enum(item) => self.define_top_level(&item.name, None),
            Item::Interface(item) => self.define_top_level(&item.name, None),
            Item::Function(item) => {
                self.define_top_level(&item.name, item.source_location.clone());
                if let Some(body) = &item.body {
                    self.check_function_body(&item.params, body);
                }
            }
            Item::TypeAlias(item) => self.define_top_level(&item.name, None),
            Item::Const(item) => self.define_top_level(&item.name, None),
            Item::Static(item) => self.define_top_level(&item.name, None),
            Item::Extern(block) => {
                for function in &block.functions {
                    self.define_top_level(&function.name, None);
                }
            }
            Item::Impl(item) => {
                for method in &item.methods {
                    if let Some(body) = &method.body {
                        self.check_function_body(&method.params, body);
                    }
                }
            }
            Item::Test(item) => {
                self.test_count += 1;
                self.check_block(&item.body, &mut HashSet::new());
            }
            Item::Module(_) | Item::Use(_) | Item::Import(_) => {}
        }
    }

    fn define_top_level(&mut self, name: &str, location: Option<String>) {
        if self.top_level.insert(name.to_string(), ()).is_some() {
            self.error_at(
                location,
                Some("MO1001"),
                format!("duplicate top-level symbol `{name}`"),
            );
        } else {
            self.enabled_symbols.insert(name.to_string());
        }
    }

    fn check_function_body(&mut self, params: &[Param], body: &Block) {
        let mut locals = HashSet::new();
        for param in params {
            let name = normalize_param_name(&param.name);
            if !name.is_empty() && !locals.insert(name.clone()) {
                self.error(format!("duplicate local binding `{name}`"));
            }
        }
        self.check_block(body, &mut locals);
    }

    fn check_block(&mut self, block: &Block, locals: &mut HashSet<String>) {
        for stmt in &block.statements {
            self.check_stmt(stmt, locals);
        }
    }

    fn check_stmt(&mut self, stmt: &Stmt, locals: &mut HashSet<String>) {
        match &stmt.data {
            StmtData::Let(let_stmt) => {
                if !locals.insert(let_stmt.name.clone()) {
                    self.error(format!("duplicate local binding `{}`", let_stmt.name));
                }
                if let Some(expr) = &let_stmt.value {
                    self.check_expr(expr, locals);
                }
            }
            StmtData::Return(Some(expr)) | StmtData::Break(Some(expr)) | StmtData::Expr(expr) => {
                self.check_expr(expr, locals)
            }
            StmtData::If(control) | StmtData::While(control) => {
                if let Some(condition) = &control.condition {
                    self.check_expr(condition, locals);
                }
                self.check_child_block(&control.body, locals);
            }
            StmtData::Match(match_expr) => self.check_match(match_expr, locals),
            StmtData::For(for_stmt) => {
                self.check_expr(&for_stmt.iterator, locals);
                let mut child = locals.clone();
                for binding in pattern_bindings(&for_stmt.pattern) {
                    if !child.insert(binding.clone()) {
                        self.error(format!("duplicate local binding `{binding}`"));
                    }
                }
                self.check_block(&for_stmt.body, &mut child);
            }
            StmtData::Loop(block) | StmtData::Unsafe(block) => {
                self.check_child_block(block, locals)
            }
            StmtData::Return(None) | StmtData::Break(None) | StmtData::Continue | StmtData::Raw => {
            }
        }
    }

    fn check_child_block(&mut self, block: &Block, locals: &HashSet<String>) {
        let mut child = locals.clone();
        self.check_block(block, &mut child);
    }

    fn check_expr(&mut self, expr: &Expr, locals: &mut HashSet<String>) {
        match expr {
            Expr::Unary(expr) => self.check_expr(&expr.expr, locals),
            Expr::Mut(expr) | Expr::Await(expr) | Expr::Try(expr) => self.check_expr(expr, locals),
            Expr::Binary(expr) => {
                self.check_expr(&expr.left, locals);
                self.check_expr(&expr.right, locals);
            }
            Expr::Index(expr) => {
                self.check_expr(&expr.target, locals);
                self.check_expr(&expr.index, locals);
            }
            Expr::Call(expr) => {
                self.check_expr(&expr.callee, locals);
                for arg in &expr.args {
                    self.check_expr(arg, locals);
                }
            }
            Expr::Member(expr) => self.check_expr(&expr.target, locals),
            Expr::Struct(expr) => {
                for field in &expr.fields {
                    if let Some(value) = &field.value {
                        self.check_expr(value, locals);
                    }
                }
            }
            Expr::Object(expr) => {
                for field in &expr.fields {
                    self.check_expr(&field.value, locals);
                }
            }
            Expr::Closure(expr) => {
                let mut child = locals.clone();
                for param in &expr.params {
                    let name = normalize_param_name(&param.name);
                    if !name.is_empty() && !child.insert(name.clone()) {
                        self.error(format!("duplicate local binding `{name}`"));
                    }
                }
                self.check_block(&expr.body, &mut child);
            }
            Expr::Match(expr) => self.check_match(expr, locals),
            Expr::If(expr) => {
                self.check_expr(&expr.condition, locals);
                self.check_child_block(&expr.then_branch, locals);
                if let Some(else_branch) = &expr.else_branch {
                    self.check_child_block(else_branch, locals);
                }
            }
            Expr::Block(block) => self.check_child_block(block, locals),
            Expr::Missing | Expr::Ident(_) | Expr::Literal(_) | Expr::Path(_) | Expr::Raw(_) => {}
        }
    }

    fn check_match(&mut self, expr: &MatchExpr, locals: &mut HashSet<String>) {
        self.check_expr(&expr.value, locals);
        for arm in &expr.arms {
            let mut child = locals.clone();
            for binding in pattern_bindings(&arm.pattern) {
                child.insert(binding);
            }
            self.check_expr(&arm.body, &mut child);
        }
    }

    fn error(&mut self, message: String) {
        self.error_at(None, None, message);
    }

    fn error_at(&mut self, location: Option<String>, code: Option<&str>, message: String) {
        self.diagnostics.push(Diagnostic {
            message,
            location,
            code: code.map(ToOwned::to_owned),
        });
    }

    fn finish(self) -> Result<CheckReport, Vec<Diagnostic>> {
        if self.diagnostics.is_empty() {
            Ok(CheckReport {
                enabled_symbols: self.enabled_symbols.into_iter().collect(),
                test_count: self.test_count,
            })
        } else {
            Err(self.diagnostics)
        }
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

pub fn target_predicate_matches(predicate: &str, target: &Target) -> bool {
    let predicate = predicate.trim();
    if let Some(inner) = call_arg(predicate, "all") {
        return split_args(inner)
            .iter()
            .all(|arg| target_predicate_matches(arg, target));
    }
    if let Some(inner) = call_arg(predicate, "any") {
        return split_args(inner)
            .iter()
            .any(|arg| target_predicate_matches(arg, target));
    }
    if let Some(inner) = call_arg(predicate, "not") {
        return !target_predicate_matches(inner, target);
    }
    target.has(predicate.trim_start_matches('.'))
}

fn call_arg<'a>(predicate: &'a str, name: &str) -> Option<&'a str> {
    let prefix = format!("{name}(");
    predicate
        .strip_prefix(&prefix)
        .and_then(|rest| rest.strip_suffix(')'))
}

fn split_args(args: &str) -> Vec<String> {
    let mut output = Vec::new();
    let mut depth = 0usize;
    let mut current = String::new();
    for ch in args.chars() {
        match ch {
            '(' => {
                depth += 1;
                current.push(ch);
            }
            ')' => {
                depth = depth.saturating_sub(1);
                current.push(ch);
            }
            ',' if depth == 0 => {
                output.push(current.trim().to_string());
                current.clear();
            }
            _ => current.push(ch),
        }
    }
    if !current.trim().is_empty() {
        output.push(current.trim().to_string());
    }
    output
}
