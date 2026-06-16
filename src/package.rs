use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::ast::*;
use crate::semantics::{Diagnostic, Target};
use crate::span::{line_column, Span};
use crate::{Lexer, Parser};

#[derive(Debug, Clone, PartialEq, Eq)]
struct ImportSpec {
    path: String,
    selector: ImportSelector,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ImportSelector {
    Names(Vec<String>),
    Namespace(String),
}

#[derive(Debug, Clone)]
struct LoadedFile {
    imports: Vec<Item>,
    local: Vec<Item>,
}

#[derive(Debug, Clone)]
struct ImportLoad {
    items: Vec<Item>,
    namespace: Option<NamespaceRewrite>,
}

#[derive(Debug, Clone)]
struct NamespaceRewrite {
    alias: String,
    exports: HashSet<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SourceErrorKind {
    Lex,
    Parse,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SourceError {
    kind: SourceErrorKind,
    message: String,
    span: Span,
}

struct PackageLoader {
    stack: Vec<PathBuf>,
    emitted_items: HashSet<String>,
    emitted_directive_items: HashSet<String>,
    package_roots: HashMap<String, PathBuf>,
    native: NativeLinkConfig,
    seen_manifests: HashSet<PathBuf>,
    target: Target,
}

impl Default for PackageLoader {
    fn default() -> Self {
        Self {
            stack: Vec::new(),
            emitted_items: HashSet::new(),
            emitted_directive_items: HashSet::new(),
            package_roots: HashMap::new(),
            native: NativeLinkConfig::default(),
            seen_manifests: HashSet::new(),
            target: Target::host(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NativeLinkConfig {
    pub static_libraries: Vec<PathBuf>,
    pub objects: Vec<PathBuf>,
    pub library_paths: Vec<PathBuf>,
    pub libraries: Vec<String>,
    pub link_args: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct LoadedPackage {
    pub program: Program,
    pub native: NativeLinkConfig,
}

impl NativeLinkConfig {
    fn merge(&mut self, other: NativeLinkConfig) {
        extend_unique_path(&mut self.static_libraries, other.static_libraries);
        extend_unique_path(&mut self.objects, other.objects);
        extend_unique_path(&mut self.library_paths, other.library_paths);
        extend_unique_string(&mut self.libraries, other.libraries);
        self.link_args.extend(other.link_args);
    }
}

#[derive(Debug, Clone, Default)]
struct ParsedManifest {
    package_roots: HashMap<String, PathBuf>,
    native: NativeLinkConfig,
    scripts: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestScripts {
    pub manifest: PathBuf,
    pub manifest_dir: PathBuf,
    pub scripts: HashMap<String, String>,
}

pub fn load_package(entry: &Path) -> Result<Program, Vec<Diagnostic>> {
    load_package_with_metadata(entry, &Target::host()).map(|loaded| loaded.program)
}

pub fn load_package_with_metadata(
    entry: &Path,
    target: &Target,
) -> Result<LoadedPackage, Vec<Diagnostic>> {
    let mut loader = PackageLoader {
        package_roots: HashMap::new(),
        target: target.clone(),
        ..PackageLoader::default()
    };
    loader.load_nearest_manifest(entry, target)?;
    let loaded = loader.load_file(entry)?;
    let mut items = Vec::new();
    loader.push_unique_items(&mut items, loaded.imports);
    items.extend(loaded.local);
    Ok(LoadedPackage {
        program: Program { items },
        native: loader.native,
    })
}

pub fn load_manifest_scripts(entry: &Path) -> Result<ManifestScripts, Vec<Diagnostic>> {
    let Some(manifest) = find_manifest(entry) else {
        return Err(vec![diagnostic(format!(
            "no mo.toml found for `{}`",
            entry.display()
        ))]);
    };
    let manifest = normalize_existing_path(&manifest)?;
    let manifest_dir = manifest
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    let parsed = parse_manifest(&manifest, &Target::host())?;
    Ok(ManifestScripts {
        manifest,
        manifest_dir,
        scripts: parsed.scripts,
    })
}

impl PackageLoader {
    fn load_file(&mut self, path: &Path) -> Result<LoadedFile, Vec<Diagnostic>> {
        let path = normalize_existing_path(path)?;
        let target = self.target.clone();
        self.load_nearest_manifest(&path, &target)?;
        if self.stack.contains(&path) {
            return Err(vec![diagnostic(format!(
                "cyclic import involving `{}`",
                path.display()
            ))]);
        }
        self.stack.push(path.clone());

        let source = fs::read_to_string(&path)
            .map_err(|err| vec![diagnostic(format!("{}: {err}", path.display()))])?;
        let mut program = parse_source(&source).map_err(|error| {
            let (line, column) = line_column(&source, error.span.start);
            let kind = match error.kind {
                SourceErrorKind::Lex => "lex",
                SourceErrorKind::Parse => "parse",
            };
            let code = match error.kind {
                SourceErrorKind::Lex => "MO0001",
                SourceErrorKind::Parse => "MO0002",
            };
            vec![diagnostic(format!(
                "{}:{line}:{column}: {kind} error [{code}]: {}",
                path.display(),
                error.message
            ))]
        })?;
        annotate_source_locations(&mut program.items, &path, &source);

        let mut imports = Vec::new();
        let mut namespace_rewrites = Vec::new();
        let mut local = Vec::new();
        let mut module = None;
        let base_dir = path.parent().unwrap_or_else(|| Path::new("."));

        let mut raw_imports = Vec::new();
        for item in program.items {
            match item {
                Item::Use(text) => local.push(Item::Use(text)),
                Item::Import(text) => raw_imports.push(text),
                Item::Module(path) => module = Some(path),
                other => local.push(other),
            }
        }

        for text in raw_imports {
            if let Some(spec) = parse_named_import(&text) {
                let loaded = self.load_import(base_dir, &spec)?;
                imports.extend(loaded.items);
                if let Some(namespace) = loaded.namespace {
                    namespace_rewrites.push(namespace);
                }
            } else {
                return Err(vec![diagnostic(format!(
                    "unsupported import syntax `{text}`; use `import * as name from \"path\"` or `import {{ Name }} from \"path\"`"
                ))]);
            }
        }

        local.insert(
            0,
            Item::Module(module.unwrap_or_else(|| synthetic_module_path(&path))),
        );

        for item in &mut local {
            rewrite_namespaces_in_item(item, &namespace_rewrites)?;
        }

        self.stack.pop();
        Ok(LoadedFile { imports, local })
    }

    fn load_import(
        &mut self,
        base_dir: &Path,
        spec: &ImportSpec,
    ) -> Result<ImportLoad, Vec<Diagnostic>> {
        let target = self.resolve_import_path(base_dir, &spec.path)?;
        let loaded = self.load_file(&target)?;
        let public_exports = exported_names(&loaded.local);
        let mut items = loaded.imports.clone();

        match &spec.selector {
            ImportSelector::Names(names) => {
                let mut missing = Vec::new();
                let mut private = Vec::new();
                items.extend(module_items(&loaded.local));
                let selected = names.iter().cloned().collect::<HashSet<_>>();
                let local_names = loaded
                    .local
                    .iter()
                    .filter_map(item_name)
                    .map(ToOwned::to_owned)
                    .collect::<HashSet<_>>();
                let dependency_names = local_names
                    .difference(&selected)
                    .cloned()
                    .collect::<HashSet<_>>();
                let implementation_alias = implementation_alias(&target);
                items.extend(namespace_items(
                    &implementation_alias,
                    loaded
                        .local
                        .iter()
                        .filter(|item| !matches!(item, Item::Module(_)))
                        .cloned()
                        .collect(),
                ));
                items.extend(module_items(&loaded.local));
                for name in names {
                    if let Some(item) = loaded
                        .local
                        .iter()
                        .find(|item| item_name(item).is_some_and(|item_name| item_name == name))
                    {
                        if item_is_public(item) {
                            let mut item = item.clone();
                            rename_local_refs_in_item(
                                &mut item,
                                &implementation_alias,
                                &dependency_names,
                            );
                            items.push(item);
                        } else {
                            private.push(name.clone());
                        }
                    } else {
                        missing.push(name.clone());
                    }
                }
                report_import_errors(&target, &missing, &private)?;
                Ok(ImportLoad {
                    items,
                    namespace: None,
                })
            }
            ImportSelector::Namespace(alias) => {
                items.extend(namespace_items(alias, loaded.local));
                Ok(ImportLoad {
                    items,
                    namespace: Some(NamespaceRewrite {
                        alias: alias.clone(),
                        exports: public_exports,
                    }),
                })
            }
        }
    }

    fn push_unique_items(&mut self, output: &mut Vec<Item>, items: Vec<Item>) {
        for item in items {
            if let Item::Directive(mut directive) = item {
                let predicate = directive.args.clone();
                directive.items.retain(|item| {
                    let Some(name) = item_name(item) else {
                        return true;
                    };
                    self.emitted_directive_items
                        .insert(format!("{predicate}:{name}"))
                });
                if !directive.items.is_empty() {
                    output.push(Item::Directive(directive));
                }
                continue;
            }
            if let Item::Extern(mut block) = item {
                block
                    .functions
                    .retain(|function| self.emitted_items.insert(function.name.clone()));
                if !block.functions.is_empty() {
                    output.push(Item::Extern(block));
                }
                continue;
            }
            let Some(name) = item_name(&item) else {
                output.push(item);
                continue;
            };
            if self.emitted_items.insert(name.to_string()) {
                output.push(item);
            }
        }
    }

    fn load_nearest_manifest(
        &mut self,
        path: &Path,
        target: &Target,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(manifest) = find_manifest(path) else {
            return Ok(());
        };
        let manifest = normalize_existing_path(&manifest)?;
        if !self.seen_manifests.insert(manifest.clone()) {
            return Ok(());
        }
        let parsed = parse_manifest(&manifest, target)?;
        for (name, root) in parsed.package_roots {
            self.package_roots.insert(name, root);
        }
        self.native.merge(parsed.native);
        Ok(())
    }
}

fn annotate_source_locations(items: &mut [Item], path: &Path, source: &str) {
    for item in items {
        match item {
            Item::Function(function) => {
                let (line, column) = line_column(source, function.span.start);
                function.source_location = Some(format!("{}:{line}:{column}", path.display()));
            }
            Item::Directive(directive) => {
                annotate_source_locations(&mut directive.items, path, source)
            }
            Item::Struct(item) => {
                for method in &mut item.methods {
                    let (line, column) = line_column(source, method.span.start);
                    method.source_location = Some(format!("{}:{line}:{column}", path.display()));
                }
            }
            Item::Impl(item) => {
                for method in &mut item.methods {
                    let (line, column) = line_column(source, method.span.start);
                    method.source_location = Some(format!("{}:{line}:{column}", path.display()));
                }
            }
            _ => {}
        }
    }
}

fn module_items(items: &[Item]) -> impl Iterator<Item = Item> + '_ {
    items
        .iter()
        .filter(|item| matches!(item, Item::Module(_)))
        .cloned()
}

fn namespace_items(alias: &str, items: Vec<Item>) -> Vec<Item> {
    let local_names = items
        .iter()
        .filter_map(item_name)
        .map(ToOwned::to_owned)
        .collect::<HashSet<_>>();

    let mut output = vec![Item::Module(crate::ast::Path {
        segments: vec![alias.to_string()],
    })];
    output.extend(items.into_iter().map(|mut item| {
        rename_local_refs_in_item(&mut item, alias, &local_names);
        rename_item_symbol(&mut item, alias);
        item
    }));
    output
}

fn parse_source(source: &str) -> Result<Program, SourceError> {
    let tokens = Lexer::new(source).tokenize().map_err(|err| SourceError {
        kind: SourceErrorKind::Lex,
        message: err.message,
        span: err.span,
    })?;

    let mut parser = Parser::new(tokens);
    parser.parse_program().map_err(|err| SourceError {
        kind: SourceErrorKind::Parse,
        message: err.message,
        span: err.span,
    })
}

fn parse_named_import(text: &str) -> Option<ImportSpec> {
    let lower = text.to_ascii_lowercase();
    let marker = " from ";
    let index = lower.rfind(marker)?;
    let names_text = text[..index].trim();
    let path_text = text[index + marker.len()..].trim();
    let path = compact_path(path_text);
    if !is_loadable_path(&path) {
        return None;
    }

    let selector = if let Some(alias) = names_text.strip_prefix("* as ") {
        let alias = normalize_import_name(alias);
        if alias.is_empty() {
            return None;
        }
        ImportSelector::Namespace(alias)
    } else if let Some(raw_names) = names_text
        .strip_prefix('{')
        .and_then(|value| value.strip_suffix('}'))
    {
        let names = raw_names
            .split(',')
            .map(normalize_import_name)
            .filter(|name| !name.is_empty())
            .collect::<Vec<_>>();
        if names.is_empty() {
            return None;
        }
        ImportSelector::Names(names)
    } else {
        return None;
    };

    Some(ImportSpec { path, selector })
}

impl PackageLoader {
    fn resolve_import_path(&self, base_dir: &Path, path: &str) -> Result<PathBuf, Vec<Diagnostic>> {
        let path = strip_quotes(path);
        let candidate = if path.starts_with("std/") {
            std_package_root().join(path.strip_prefix("std/").unwrap())
        } else if path.starts_with("core/") {
            core_package_root().join(path.strip_prefix("core/").unwrap())
        } else if path.starts_with("alloc/") {
            alloc_package_root().join(path.strip_prefix("alloc/").unwrap())
        } else if path.starts_with("lib/") {
            userland_package_root().join(path.strip_prefix("lib/").unwrap())
        } else if let Some((package, rest)) = split_package_import(path) {
            self.package_roots
                .get(package)
                .map(|root| root.join(rest))
                .unwrap_or_else(|| base_dir.join(path))
        } else if let Some(root) = self.package_roots.get(path) {
            root.clone()
        } else {
            base_dir.join(path)
        };
        let file = if candidate.is_dir() {
            candidate.join("index.mo")
        } else if candidate.extension().is_some() {
            candidate
        } else {
            candidate.with_extension("mo")
        };
        normalize_existing_path(&file)
    }
}

fn split_package_import(path: &str) -> Option<(&str, &str)> {
    let (package, rest) = path.split_once('/')?;
    (!package.is_empty() && !rest.is_empty()).then_some((package, rest))
}

fn find_manifest(entry: &Path) -> Option<PathBuf> {
    let mut dir = if entry.is_dir() {
        entry.to_path_buf()
    } else {
        entry.parent()?.to_path_buf()
    };
    loop {
        let candidate = dir.join("mo.toml");
        if candidate.exists() {
            return Some(candidate);
        }
        if !dir.pop() {
            return None;
        }
    }
}

fn parse_manifest(manifest: &Path, target: &Target) -> Result<ParsedManifest, Vec<Diagnostic>> {
    let source = fs::read_to_string(manifest)
        .map_err(|err| vec![diagnostic(format!("{}: {err}", manifest.display()))])?;
    let manifest_dir = manifest.parent().unwrap_or_else(|| Path::new("."));
    let mut section = String::new();
    let mut package_name = None;
    let mut package_root = None;
    let mut dependencies = HashMap::new();
    let mut native = NativeLinkConfig::default();
    let mut scripts = HashMap::new();

    for line in manifest_logical_lines(&source) {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            section = line
                .trim_start_matches('[')
                .trim_end_matches(']')
                .trim()
                .to_string();
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = strip_quotes(value.trim()).to_string();
        match section.as_str() {
            "package" if key == "name" => package_name = Some(value),
            "package" if key == "root" => package_root = Some(value),
            "dependencies" if !key.is_empty() => {
                dependencies.insert(key.to_string(), value);
            }
            section if target_dependency_section_matches(section, target) && !key.is_empty() => {
                dependencies.insert(key.to_string(), value);
            }
            "scripts" if !key.is_empty() => {
                scripts.insert(key.to_string(), value);
            }
            section if native_section_matches(section, target) => {
                parse_native_manifest_value(&mut native, manifest_dir, key, value.trim())?;
            }
            _ => {}
        }
    }

    let mut roots = HashMap::new();
    if let Some(name) = package_name {
        let root = package_root.unwrap_or_else(|| ".".to_string());
        roots.insert(name, normalize_manifest_root(manifest_dir, &root)?);
    }
    for (name, root) in dependencies {
        roots.insert(name, normalize_manifest_root(manifest_dir, &root)?);
    }
    Ok(ParsedManifest {
        package_roots: roots,
        native,
        scripts,
    })
}

fn manifest_logical_lines(source: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut pending = None::<String>;
    let mut bracket_depth = 0i32;

    for raw_line in source.lines() {
        let line = raw_line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }

        if let Some(current) = pending.as_mut() {
            current.push(' ');
            current.push_str(line);
            bracket_depth += manifest_bracket_delta(line);
            if bracket_depth <= 0 {
                lines.push(pending.take().expect("pending manifest line"));
            }
            continue;
        }

        bracket_depth = manifest_bracket_delta(line);
        if line.contains('=') && bracket_depth > 0 {
            pending = Some(line.to_string());
        } else {
            lines.push(line.to_string());
        }
    }

    if let Some(line) = pending {
        lines.push(line);
    }

    lines
}

fn manifest_bracket_delta(line: &str) -> i32 {
    let opens = line.chars().filter(|ch| *ch == '[').count() as i32;
    let closes = line.chars().filter(|ch| *ch == ']').count() as i32;
    opens - closes
}

fn normalize_manifest_root(manifest_dir: &Path, root: &str) -> Result<PathBuf, Vec<Diagnostic>> {
    normalize_existing_path(&manifest_dir.join(root))
}

fn parse_native_manifest_value(
    native: &mut NativeLinkConfig,
    manifest_dir: &Path,
    key: &str,
    value: &str,
) -> Result<(), Vec<Diagnostic>> {
    match key {
        "static_libraries" => {
            let values = parse_manifest_string_array(value)?;
            for value in values {
                native
                    .static_libraries
                    .push(normalize_manifest_file(manifest_dir, &value)?);
            }
        }
        "objects" => {
            let values = parse_manifest_string_array(value)?;
            for value in values {
                native
                    .objects
                    .push(normalize_manifest_file(manifest_dir, &value)?);
            }
        }
        "library_paths" => {
            let values = parse_manifest_string_array(value)?;
            for value in values {
                native
                    .library_paths
                    .push(normalize_manifest_native_dir(manifest_dir, &value)?);
            }
        }
        "libraries" => native.libraries.extend(parse_manifest_string_array(value)?),
        "link_args" => native.link_args.extend(parse_manifest_string_array(value)?),
        _ => {}
    }
    Ok(())
}

fn normalize_manifest_file(manifest_dir: &Path, path: &str) -> Result<PathBuf, Vec<Diagnostic>> {
    Ok(resolve_manifest_path(manifest_dir, path))
}

fn normalize_manifest_native_dir(
    manifest_dir: &Path,
    path: &str,
) -> Result<PathBuf, Vec<Diagnostic>> {
    Ok(resolve_manifest_path(manifest_dir, path))
}

fn resolve_manifest_path(manifest_dir: &Path, path: &str) -> PathBuf {
    let path = Path::new(path);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        manifest_dir.join(path)
    }
}

fn parse_manifest_string_array(value: &str) -> Result<Vec<String>, Vec<Diagnostic>> {
    let value = value.trim();
    let Some(inner) = value
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
    else {
        return Err(vec![diagnostic(format!(
            "manifest native values must be string arrays, got `{value}`"
        ))]);
    };
    let mut values = Vec::new();
    for raw in inner.split(',') {
        let item = raw.trim();
        if item.is_empty() {
            continue;
        }
        if item.starts_with('"') && item.ends_with('"') && item.len() >= 2 {
            values.push(item[1..item.len() - 1].to_string());
        } else if item.starts_with('\'') && item.ends_with('\'') && item.len() >= 2 {
            values.push(item[1..item.len() - 1].to_string());
        } else {
            return Err(vec![diagnostic(format!(
                "manifest native array item must be quoted, got `{item}`"
            ))]);
        }
    }
    Ok(values)
}

fn native_section_matches(section: &str, target: &Target) -> bool {
    let Some(rest) = section.strip_prefix("native") else {
        return false;
    };
    if rest.is_empty() {
        return true;
    }
    let Some(symbols) = rest.strip_prefix('.') else {
        return false;
    };
    symbols
        .split('.')
        .filter(|symbol| !symbol.is_empty())
        .all(|symbol| target.has(symbol))
}

fn target_dependency_section_matches(section: &str, target: &Target) -> bool {
    let Some(rest) = section
        .strip_prefix("target.")
        .and_then(|rest| rest.strip_suffix(".dependencies"))
    else {
        return false;
    };
    rest.split('.')
        .filter(|symbol| !symbol.is_empty())
        .all(|symbol| target.has(symbol))
}

fn extend_unique_path(output: &mut Vec<PathBuf>, input: Vec<PathBuf>) {
    for item in input {
        if !output.contains(&item) {
            output.push(item);
        }
    }
}

fn extend_unique_string(output: &mut Vec<String>, input: Vec<String>) {
    for item in input {
        if !output.contains(&item) {
            output.push(item);
        }
    }
}

fn std_package_root() -> PathBuf {
    mo_package_root().join("std")
}

fn core_package_root() -> PathBuf {
    mo_package_root().join("core")
}

fn alloc_package_root() -> PathBuf {
    mo_package_root().join("alloc")
}

fn userland_package_root() -> PathBuf {
    mo_package_root().join("lib")
}

fn mo_package_root() -> PathBuf {
    if let Some(root) = env::var_os("MO_ROOT") {
        return PathBuf::from(root);
    }

    if let Ok(exe) = env::current_exe() {
        if let Some(root) = exe.parent() {
            if root.join("std").exists()
                && root.join("core").exists()
                && root.join("alloc").exists()
            {
                return root.to_path_buf();
            }
        }
    }

    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn normalize_existing_path(path: &Path) -> Result<PathBuf, Vec<Diagnostic>> {
    path.canonicalize()
        .map_err(|err| vec![diagnostic(format!("{}: {err}", path.display()))])
}

fn synthetic_module_path(path: &Path) -> crate::ast::Path {
    let stem_path = path.with_extension("");
    let segments = stem_path
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => value.to_str().map(sanitize_module_segment),
            _ => None,
        })
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    crate::ast::Path { segments }
}

fn sanitize_module_segment(segment: &str) -> String {
    segment
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn report_import_errors(
    target: &Path,
    missing: &[String],
    private: &[String],
) -> Result<(), Vec<Diagnostic>> {
    if missing.is_empty() && private.is_empty() {
        return Ok(());
    }
    let mut diagnostics = Vec::new();
    if !missing.is_empty() {
        diagnostics.push(diagnostic(format!(
            "`{}` does not export {}; available public export(s): {}",
            target.display(),
            format_backtick_names(missing),
            format_available_exports(target)
        )));
    }
    if !private.is_empty() {
        diagnostics.push(diagnostic(format!(
            "`{}` has private item(s) {}; add `pub` to export them",
            target.display(),
            format_backtick_names(private)
        )));
    }
    Err(diagnostics)
}

fn format_backtick_names(names: &[String]) -> String {
    names
        .iter()
        .map(|name| format!("`{name}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn format_available_exports(target: &Path) -> String {
    let Ok(source) = fs::read_to_string(target) else {
        return "<unavailable>".to_string();
    };
    let Ok(program) = parse_source(&source) else {
        return "<unavailable>".to_string();
    };
    let mut names = program
        .items
        .iter()
        .filter(|item| item_is_public(item))
        .filter_map(item_name)
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    names.sort();
    if names.is_empty() {
        "<none>".to_string()
    } else {
        format_backtick_names(&names)
    }
}

fn format_export_set(exports: &HashSet<String>) -> String {
    let mut names = exports.iter().cloned().collect::<Vec<_>>();
    names.sort();
    if names.is_empty() {
        "<none>".to_string()
    } else {
        format_backtick_names(&names)
    }
}

fn exported_names(items: &[Item]) -> HashSet<String> {
    items
        .iter()
        .filter(|item| item_is_public(item))
        .filter_map(item_name)
        .map(ToOwned::to_owned)
        .collect()
}

fn item_name(item: &Item) -> Option<&str> {
    match item {
        Item::Struct(item) => Some(&item.name),
        Item::Enum(item) => Some(&item.name),
        Item::Interface(item) => Some(&item.name),
        Item::Function(item) => Some(&item.name),
        Item::TypeAlias(item) => Some(&item.name),
        Item::Const(item) => Some(&item.name),
        Item::Static(item) => Some(&item.name),
        Item::Test(item) => Some(&item.name),
        _ => None,
    }
}

fn item_is_public(item: &Item) -> bool {
    match item {
        Item::Struct(item) => item.public,
        Item::Enum(item) => item.public,
        Item::Interface(item) => item.public,
        Item::Function(item) => item.public,
        Item::TypeAlias(item) => item.public,
        Item::Const(item) => item.public,
        Item::Static(item) => item.public,
        Item::Extern(_) | Item::Impl(_) => true,
        _ => false,
    }
}

fn rename_item_symbol(item: &mut Item, alias: &str) {
    match item {
        Item::Struct(item) => item.name = namespace_symbol(alias, &item.name),
        Item::Enum(item) => item.name = namespace_symbol(alias, &item.name),
        Item::Interface(item) => item.name = namespace_symbol(alias, &item.name),
        Item::Function(item) => item.name = namespace_symbol(alias, &item.name),
        Item::TypeAlias(item) => item.name = namespace_symbol(alias, &item.name),
        Item::Const(item) => item.name = namespace_symbol(alias, &item.name),
        Item::Static(item) => item.name = namespace_symbol(alias, &item.name),
        Item::Test(item) => item.name = namespace_symbol(alias, &item.name),
        _ => {}
    }
}

fn rename_local_refs_in_item(item: &mut Item, alias: &str, local_names: &HashSet<String>) {
    match item {
        Item::Struct(item) => {
            for field in &mut item.fields {
                rewrite_type_local_refs(&mut field.ty_expr, alias, local_names);
            }
            for method in &mut item.methods {
                rewrite_function_local_refs(method, alias, local_names);
            }
        }
        Item::Enum(_) => {}
        Item::Interface(item) => {
            for method in &mut item.methods {
                rewrite_signature_local_refs(method, alias, local_names);
            }
        }
        Item::Impl(item) => {
            if local_names.contains(&item.target) {
                item.target = namespace_symbol(alias, &item.target);
            }
            if let Some(interface) = &mut item.interface {
                if local_names.contains(interface) {
                    *interface = namespace_symbol(alias, interface);
                }
            }
            for method in &mut item.methods {
                rewrite_function_local_refs(method, alias, local_names);
            }
        }
        Item::Function(item) => rewrite_function_local_refs(item, alias, local_names),
        Item::TypeAlias(item) => rewrite_type_local_refs(&mut item.value_expr, alias, local_names),
        Item::Const(_) | Item::Static(_) | Item::Extern(_) | Item::Directive(_) => {}
        Item::Module(_) | Item::Use(_) | Item::Import(_) | Item::Test(_) => {}
    }
}

fn rewrite_function_local_refs(
    function: &mut FunctionItem,
    alias: &str,
    local_names: &HashSet<String>,
) {
    for param in &mut function.params {
        if let Some(ty) = &mut param.ty_expr {
            rewrite_type_local_refs(ty, alias, local_names);
        }
    }
    if let Some(return_type) = &mut function.return_type_expr {
        rewrite_type_local_refs(return_type, alias, local_names);
    }
    if let Some(body) = &mut function.body {
        rewrite_block_local_refs(body, alias, local_names);
    }
}

fn rewrite_signature_local_refs(
    signature: &mut FunctionSignature,
    alias: &str,
    local_names: &HashSet<String>,
) {
    for param in &mut signature.params {
        if let Some(ty) = &mut param.ty_expr {
            rewrite_type_local_refs(ty, alias, local_names);
        }
    }
    if let Some(return_type) = &mut signature.return_type_expr {
        rewrite_type_local_refs(return_type, alias, local_names);
    }
}

fn rewrite_block_local_refs(block: &mut Block, alias: &str, local_names: &HashSet<String>) {
    for stmt in &mut block.statements {
        match &mut stmt.data {
            StmtData::Let(stmt) => {
                if let Some(ty) = &mut stmt.ty_expr {
                    rewrite_type_local_refs(ty, alias, local_names);
                }
                if let Some(value) = &mut stmt.value {
                    rewrite_expr_local_refs(value, alias, local_names);
                }
            }
            StmtData::Return(Some(expr)) | StmtData::Break(Some(expr)) | StmtData::Expr(expr) => {
                rewrite_expr_local_refs(expr, alias, local_names)
            }
            StmtData::If(control) | StmtData::While(control) => {
                if let Some(condition) = &mut control.condition {
                    rewrite_expr_local_refs(condition, alias, local_names);
                }
                rewrite_block_local_refs(&mut control.body, alias, local_names);
            }
            StmtData::Match(expr) => rewrite_match_local_refs(expr, alias, local_names),
            StmtData::For(stmt) => {
                rewrite_expr_local_refs(&mut stmt.iterator, alias, local_names);
                rewrite_block_local_refs(&mut stmt.body, alias, local_names);
            }
            StmtData::Loop(block) | StmtData::Unsafe(block) => {
                rewrite_block_local_refs(block, alias, local_names)
            }
            StmtData::Return(None) | StmtData::Break(None) | StmtData::Continue | StmtData::Raw => {
            }
        }
    }
}

fn rewrite_match_local_refs(expr: &mut MatchExpr, alias: &str, local_names: &HashSet<String>) {
    rewrite_expr_local_refs(&mut expr.value, alias, local_names);
    for arm in &mut expr.arms {
        rewrite_expr_local_refs(&mut arm.body, alias, local_names);
    }
}

fn rewrite_expr_local_refs(expr: &mut Expr, alias: &str, local_names: &HashSet<String>) {
    match expr {
        Expr::Ident(name) => {
            if local_names.contains(name) {
                *name = namespace_symbol(alias, name);
            }
        }
        Expr::Path(path) => {
            if let Some(first) = path.first_mut() {
                if local_names.contains(first) {
                    *first = namespace_symbol(alias, first);
                }
            }
        }
        Expr::Unary(expr) => rewrite_expr_local_refs(&mut expr.expr, alias, local_names),
        Expr::Mut(expr) | Expr::Await(expr) | Expr::Try(expr) => {
            rewrite_expr_local_refs(expr, alias, local_names)
        }
        Expr::Binary(expr) => {
            rewrite_expr_local_refs(&mut expr.left, alias, local_names);
            rewrite_expr_local_refs(&mut expr.right, alias, local_names);
        }
        Expr::Index(expr) => {
            rewrite_expr_local_refs(&mut expr.target, alias, local_names);
            rewrite_expr_local_refs(&mut expr.index, alias, local_names);
        }
        Expr::Call(expr) => {
            rewrite_expr_local_refs(&mut expr.callee, alias, local_names);
            for arg in &mut expr.args {
                rewrite_expr_local_refs(arg, alias, local_names);
            }
        }
        Expr::Member(expr) => rewrite_expr_local_refs(&mut expr.target, alias, local_names),
        Expr::Struct(expr) => {
            if local_names.contains(&expr.name) {
                expr.name = namespace_symbol(alias, &expr.name);
            }
            for field in &mut expr.fields {
                if let Some(value) = &mut field.value {
                    rewrite_expr_local_refs(value, alias, local_names);
                }
            }
        }
        Expr::Object(expr) => {
            for field in &mut expr.fields {
                rewrite_expr_local_refs(&mut field.value, alias, local_names);
            }
        }
        Expr::Closure(expr) => rewrite_block_local_refs(&mut expr.body, alias, local_names),
        Expr::Match(expr) => rewrite_match_local_refs(expr, alias, local_names),
        Expr::If(expr) => {
            rewrite_expr_local_refs(&mut expr.condition, alias, local_names);
            rewrite_block_local_refs(&mut expr.then_branch, alias, local_names);
            if let Some(else_branch) = &mut expr.else_branch {
                rewrite_block_local_refs(else_branch, alias, local_names);
            }
        }
        Expr::Block(block) => rewrite_block_local_refs(block, alias, local_names),
        Expr::Missing | Expr::Literal(_) | Expr::Raw(_) => {}
    }
}

fn rewrite_type_local_refs(ty: &mut TypeExpr, alias: &str, local_names: &HashSet<String>) {
    match ty {
        TypeExpr::Path(path) => {
            if let Some(first) = path.first_mut() {
                if local_names.contains(first) {
                    *first = namespace_symbol(alias, first);
                }
            }
        }
        TypeExpr::Generic { base, args } => {
            rewrite_type_local_refs(base, alias, local_names);
            for arg in args {
                rewrite_type_local_refs(arg, alias, local_names);
            }
        }
        TypeExpr::Tuple(items) => {
            for item in items {
                rewrite_type_local_refs(item, alias, local_names);
            }
        }
        TypeExpr::Fn {
            params,
            return_type,
            ..
        } => {
            for param in params {
                rewrite_type_local_refs(param, alias, local_names);
            }
            rewrite_type_local_refs(return_type, alias, local_names);
        }
        TypeExpr::Ref { inner, .. }
        | TypeExpr::RawPtr { inner, .. }
        | TypeExpr::Impl(inner)
        | TypeExpr::Mut(inner) => rewrite_type_local_refs(inner, alias, local_names),
        TypeExpr::Missing => {}
    }
}

fn rewrite_namespaces_in_item(
    item: &mut Item,
    namespaces: &[NamespaceRewrite],
) -> Result<(), Vec<Diagnostic>> {
    match item {
        Item::Struct(item) => {
            for field in &mut item.fields {
                rewrite_namespaces_in_type(&mut field.ty_expr, namespaces)?;
            }
            for method in &mut item.methods {
                rewrite_namespaces_in_function(method, namespaces)?;
            }
        }
        Item::Function(item) => {
            for param in &mut item.params {
                if let Some(ty) = &mut param.ty_expr {
                    rewrite_namespaces_in_type(ty, namespaces)?;
                }
            }
            if let Some(ty) = &mut item.return_type_expr {
                rewrite_namespaces_in_type(ty, namespaces)?;
            }
            if let Some(body) = &mut item.body {
                rewrite_namespaces_in_block(body, namespaces)?;
            }
        }
        Item::TypeAlias(item) => rewrite_namespaces_in_type(&mut item.value_expr, namespaces)?,
        Item::Test(item) => rewrite_namespaces_in_block(&mut item.body, namespaces)?,
        Item::Directive(directive) => {
            for item in &mut directive.items {
                rewrite_namespaces_in_item(item, namespaces)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn rewrite_namespaces_in_function(
    function: &mut FunctionItem,
    namespaces: &[NamespaceRewrite],
) -> Result<(), Vec<Diagnostic>> {
    for param in &mut function.params {
        if let Some(ty) = &mut param.ty_expr {
            rewrite_namespaces_in_type(ty, namespaces)?;
        }
    }
    if let Some(ty) = &mut function.return_type_expr {
        rewrite_namespaces_in_type(ty, namespaces)?;
    }
    if let Some(body) = &mut function.body {
        rewrite_namespaces_in_block(body, namespaces)?;
    }
    Ok(())
}

fn rewrite_namespaces_in_block(
    block: &mut Block,
    namespaces: &[NamespaceRewrite],
) -> Result<(), Vec<Diagnostic>> {
    for stmt in &mut block.statements {
        match &mut stmt.data {
            StmtData::Let(stmt) => {
                if let Some(ty) = &mut stmt.ty_expr {
                    rewrite_namespaces_in_type(ty, namespaces)?;
                }
                if let Some(value) = &mut stmt.value {
                    rewrite_namespaces_in_expr(value, namespaces)?;
                }
            }
            StmtData::Return(Some(expr)) | StmtData::Break(Some(expr)) | StmtData::Expr(expr) => {
                rewrite_namespaces_in_expr(expr, namespaces)?
            }
            StmtData::If(control) | StmtData::While(control) => {
                if let Some(condition) = &mut control.condition {
                    rewrite_namespaces_in_expr(condition, namespaces)?;
                }
                rewrite_namespaces_in_block(&mut control.body, namespaces)?;
            }
            StmtData::Match(expr) => rewrite_namespaces_in_match(expr, namespaces)?,
            StmtData::For(stmt) => {
                rewrite_namespaces_in_expr(&mut stmt.iterator, namespaces)?;
                rewrite_namespaces_in_block(&mut stmt.body, namespaces)?;
            }
            StmtData::Loop(block) | StmtData::Unsafe(block) => {
                rewrite_namespaces_in_block(block, namespaces)?
            }
            StmtData::Return(None) | StmtData::Break(None) | StmtData::Continue | StmtData::Raw => {
            }
        }
    }
    Ok(())
}

fn rewrite_namespaces_in_match(
    expr: &mut MatchExpr,
    namespaces: &[NamespaceRewrite],
) -> Result<(), Vec<Diagnostic>> {
    rewrite_namespaces_in_expr(&mut expr.value, namespaces)?;
    for arm in &mut expr.arms {
        rewrite_namespaces_in_expr(&mut arm.body, namespaces)?;
    }
    Ok(())
}

fn rewrite_namespaces_in_expr(
    expr: &mut Expr,
    namespaces: &[NamespaceRewrite],
) -> Result<(), Vec<Diagnostic>> {
    match expr {
        Expr::Call(expr) => {
            if let Expr::Member(member) = expr.callee.as_mut() {
                if let Expr::Ident(alias) = member.target.as_ref() {
                    if let Some(namespace) = namespaces
                        .iter()
                        .find(|namespace| namespace.alias == *alias)
                    {
                        if !namespace.exports.contains(&member.member) {
                            return Err(vec![diagnostic(format!(
                                "`{}` is not exported by namespace `{alias}`; available public export(s): {}",
                                member.member,
                                format_export_set(&namespace.exports)
                            ))]);
                        }
                        expr.callee =
                            Box::new(Expr::Ident(namespace_symbol(alias, &member.member)));
                    }
                }
            }
            rewrite_namespaces_in_expr(&mut expr.callee, namespaces)?;
            for arg in &mut expr.args {
                rewrite_namespaces_in_expr(arg, namespaces)?;
            }
        }
        Expr::Member(member) => rewrite_namespaces_in_expr(&mut member.target, namespaces)?,
        Expr::Unary(expr) => rewrite_namespaces_in_expr(&mut expr.expr, namespaces)?,
        Expr::Mut(expr) | Expr::Await(expr) | Expr::Try(expr) => {
            rewrite_namespaces_in_expr(expr, namespaces)?
        }
        Expr::Binary(expr) => {
            rewrite_namespaces_in_expr(&mut expr.left, namespaces)?;
            rewrite_namespaces_in_expr(&mut expr.right, namespaces)?;
        }
        Expr::Index(expr) => {
            rewrite_namespaces_in_expr(&mut expr.target, namespaces)?;
            rewrite_namespaces_in_expr(&mut expr.index, namespaces)?;
        }
        Expr::Struct(expr) => {
            for field in &mut expr.fields {
                if let Some(value) = &mut field.value {
                    rewrite_namespaces_in_expr(value, namespaces)?;
                }
            }
        }
        Expr::Object(expr) => {
            for field in &mut expr.fields {
                rewrite_namespaces_in_expr(&mut field.value, namespaces)?;
            }
        }
        Expr::Closure(expr) => {
            for param in &mut expr.params {
                if let Some(ty) = &mut param.ty_expr {
                    rewrite_namespaces_in_type(ty, namespaces)?;
                }
            }
            if let Some(ty) = &mut expr.return_type_expr {
                rewrite_namespaces_in_type(ty, namespaces)?;
            }
            rewrite_namespaces_in_block(&mut expr.body, namespaces)?;
        }
        Expr::Match(expr) => rewrite_namespaces_in_match(expr, namespaces)?,
        Expr::If(expr) => {
            rewrite_namespaces_in_expr(&mut expr.condition, namespaces)?;
            rewrite_namespaces_in_block(&mut expr.then_branch, namespaces)?;
            if let Some(else_branch) = &mut expr.else_branch {
                rewrite_namespaces_in_block(else_branch, namespaces)?;
            }
        }
        Expr::Block(block) => rewrite_namespaces_in_block(block, namespaces)?,
        Expr::Ident(_) | Expr::Path(_) | Expr::Missing | Expr::Literal(_) | Expr::Raw(_) => {}
    }
    Ok(())
}

fn rewrite_namespaces_in_type(
    ty: &mut TypeExpr,
    namespaces: &[NamespaceRewrite],
) -> Result<(), Vec<Diagnostic>> {
    match ty {
        TypeExpr::Path(path) => {
            if path.len() >= 2 {
                let alias = path[0].clone();
                let name = path[1].clone();
                if let Some(namespace) =
                    namespaces.iter().find(|namespace| namespace.alias == alias)
                {
                    if !namespace.exports.contains(&name) {
                        return Err(vec![diagnostic(format!(
                            "`{name}` is not exported by namespace `{alias}`; available public export(s): {}",
                            format_export_set(&namespace.exports)
                        ))]);
                    }
                    *path = vec![namespace_symbol(&alias, &name)];
                }
            }
        }
        TypeExpr::Generic { base, args } => {
            rewrite_namespaces_in_type(base, namespaces)?;
            for arg in args {
                rewrite_namespaces_in_type(arg, namespaces)?;
            }
        }
        TypeExpr::Tuple(items) => {
            for item in items {
                rewrite_namespaces_in_type(item, namespaces)?;
            }
        }
        TypeExpr::Fn {
            params,
            return_type,
            ..
        } => {
            for param in params {
                rewrite_namespaces_in_type(param, namespaces)?;
            }
            rewrite_namespaces_in_type(return_type, namespaces)?;
        }
        TypeExpr::Ref { inner, .. }
        | TypeExpr::RawPtr { inner, .. }
        | TypeExpr::Impl(inner)
        | TypeExpr::Mut(inner) => rewrite_namespaces_in_type(inner, namespaces)?,
        TypeExpr::Missing => {}
    }
    Ok(())
}

fn namespace_symbol(alias: &str, name: &str) -> String {
    format!("{alias}__{name}")
}

fn implementation_alias(path: &Path) -> String {
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("module");
    format!("__impl_{}", sanitize_module_segment(stem))
}

fn compact_path(text: &str) -> String {
    strip_quotes(text)
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect()
}

fn strip_quotes(text: &str) -> &str {
    text.trim()
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or_else(|| {
            text.trim()
                .strip_prefix('\'')
                .and_then(|value| value.strip_suffix('\''))
                .unwrap_or(text.trim())
        })
}

fn normalize_import_name(name: &str) -> String {
    match name.trim() {
        "Async" => "async".into(),
        "Self_" => "self".into(),
        name => name.to_string(),
    }
}

fn is_loadable_path(path: &str) -> bool {
    path.starts_with("./")
        || path.starts_with("../")
        || path.starts_with("std/")
        || path.starts_with("core/")
        || path.starts_with("alloc/")
        || path.starts_with("lib/")
        || is_manifest_import_path(path)
}

fn is_manifest_import_path(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.contains(char::is_whitespace)
        && path
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '/' | '.'))
}

fn diagnostic(message: impl Into<String>) -> Diagnostic {
    Diagnostic {
        message: message.into(),
        location: None,
        code: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_named_relative_import() {
        assert_eq!(
            parse_named_import("{ A, B } from ./mod"),
            Some(ImportSpec {
                path: "./mod".into(),
                selector: ImportSelector::Names(vec!["A".into(), "B".into()]),
            })
        );
    }

    #[test]
    fn parses_namespace_relative_import() {
        assert_eq!(
            parse_named_import("* as math from ./math"),
            Some(ImportSpec {
                path: "./math".into(),
                selector: ImportSelector::Namespace("math".into()),
            })
        );
    }

    #[test]
    fn rejects_bare_glob_relative_import() {
        assert_eq!(parse_named_import("* from ./mod"), None);
    }

    #[test]
    fn parses_std_namespace_import() {
        assert_eq!(
            parse_named_import("* as async from std/async"),
            Some(ImportSpec {
                path: "std/async".into(),
                selector: ImportSelector::Namespace("async".into()),
            })
        );
    }

    #[test]
    fn parses_lib_namespace_import() {
        assert_eq!(
            parse_named_import("* as pokemon from lib/pokemon"),
            Some(ImportSpec {
                path: "lib/pokemon".into(),
                selector: ImportSelector::Namespace("pokemon".into()),
            })
        );
    }

    #[test]
    fn parses_alloc_namespace_import() {
        assert_eq!(
            parse_named_import("* as string from alloc/string"),
            Some(ImportSpec {
                path: "alloc/string".into(),
                selector: ImportSelector::Namespace("string".into()),
            })
        );
    }

    #[test]
    fn parses_manifest_package_import() {
        assert_eq!(
            parse_named_import("* as answer from math/answer"),
            Some(ImportSpec {
                path: "math/answer".into(),
                selector: ImportSelector::Namespace("answer".into()),
            })
        );
    }

    #[test]
    fn parses_multiline_native_manifest_arrays() {
        let root = std::env::temp_dir().join(format!(
            "mo_multiline_native_manifest_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create manifest dir");
        let root = normalize_existing_path(&root).expect("normalize manifest dir");
        let manifest = root.join("mo.toml");
        std::fs::write(
            &manifest,
            r#"
[native.macos.aarch64]
static_libraries = [
    "vendor/libshim.a",
    "vendor/libdep.a",
]
link_args = [
    "-framework", "Cocoa",
    "-framework", "OpenGL",
]
"#,
        )
        .expect("write manifest");

        let parsed = parse_manifest(&manifest, &Target::macos_aarch64()).expect("parse manifest");

        assert_eq!(
            parsed.native.static_libraries,
            vec![root.join("vendor/libshim.a"), root.join("vendor/libdep.a")]
        );
        assert_eq!(
            parsed.native.link_args,
            vec!["-framework", "Cocoa", "-framework", "OpenGL"]
        );
    }

    #[test]
    fn parses_target_specific_dependencies_for_active_target() {
        let root =
            std::env::temp_dir().join(format!("mo_target_dependencies_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("packages/common")).expect("create common dir");
        std::fs::create_dir_all(root.join("packages/linux")).expect("create linux dir");
        std::fs::create_dir_all(root.join("packages/macos")).expect("create macos dir");
        let root = normalize_existing_path(&root).expect("normalize manifest dir");
        let manifest = root.join("mo.toml");
        std::fs::write(
            &manifest,
            r#"
[dependencies]
common = "packages/common"

[target.linux.dependencies]
platform = "packages/linux"

[target.macos.dependencies]
platform = "packages/macos"
"#,
        )
        .expect("write manifest");

        let linux = parse_manifest(&manifest, &Target::linux_x86_64()).expect("parse linux");
        assert_eq!(
            linux.package_roots.get("common"),
            Some(&root.join("packages/common"))
        );
        assert_eq!(
            linux.package_roots.get("platform"),
            Some(&root.join("packages/linux"))
        );

        let macos = parse_manifest(&manifest, &Target::macos_aarch64()).expect("parse macos");
        assert_eq!(
            macos.package_roots.get("platform"),
            Some(&root.join("packages/macos"))
        );
    }

    #[test]
    fn rewrites_namespace_imports_inside_target_directives() {
        let root = std::env::temp_dir().join(format!(
            "mo_target_namespace_rewrite_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create package dir");
        std::fs::write(
            root.join("util.mo"),
            r#"
pub fn value() -> Int {
    return 42
}
"#,
        )
        .expect("write util");
        std::fs::write(
            root.join("main.mo"),
            r#"
import * as util from "./util"

@target(.linux) {
    fn main() -> Int {
        return util.value()
    }
}
"#,
        )
        .expect("write main");

        let loaded = load_package_with_metadata(&root.join("main.mo"), &Target::linux_x86_64())
            .expect("load package");
        let mut found_rewrite = false;
        for item in loaded.program.items {
            if let Item::Directive(directive) = item {
                for item in directive.items {
                    if let Item::Function(function) = item {
                        let Some(body) = function.body else {
                            continue;
                        };
                        let Some(stmt) = body.statements.first() else {
                            continue;
                        };
                        if let StmtData::Return(Some(Expr::Call(call))) = &stmt.data {
                            found_rewrite = matches!(
                                call.callee.as_ref(),
                                Expr::Ident(name) if name == "util__value"
                            );
                        }
                    }
                }
            }
        }
        assert!(
            found_rewrite,
            "target directive call should be namespace-rewritten"
        );
    }

    #[test]
    fn preserves_repeated_native_link_args() {
        let mut left = NativeLinkConfig {
            link_args: vec!["-framework".into(), "Cocoa".into()],
            ..NativeLinkConfig::default()
        };
        left.merge(NativeLinkConfig {
            link_args: vec![
                "-framework".into(),
                "IOKit".into(),
                "-framework".into(),
                "CoreVideo".into(),
            ],
            ..NativeLinkConfig::default()
        });

        assert_eq!(
            left.link_args,
            vec![
                "-framework",
                "Cocoa",
                "-framework",
                "IOKit",
                "-framework",
                "CoreVideo"
            ]
        );
    }

    #[test]
    fn parses_manifest_scripts() {
        let root = std::env::temp_dir().join(format!("mo_manifest_scripts_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create manifest dir");
        let root = normalize_existing_path(&root).expect("normalize manifest dir");
        let manifest = root.join("mo.toml");
        std::fs::write(
            &manifest,
            r#"
[scripts]
prepare = "./prepare.sh"
build = "mo build app/main.mo -o /tmp/app"
"#,
        )
        .expect("write manifest");

        let loaded = load_manifest_scripts(&root).expect("load manifest scripts");

        assert_eq!(loaded.manifest, manifest);
        assert_eq!(loaded.manifest_dir, root);
        assert_eq!(
            loaded.scripts.get("prepare"),
            Some(&"./prepare.sh".to_string())
        );
        assert_eq!(
            loaded.scripts.get("build"),
            Some(&"mo build app/main.mo -o /tmp/app".to_string())
        );
    }

    #[test]
    fn rejects_legacy_selected_import_without_braces() {
        assert_eq!(parse_named_import("A, B from ./mod"), None);
    }
}
