use std::path::{Path, PathBuf};
use std::process::Command;

use crate::ast::Program;
use crate::borrow::check_borrows;
use crate::dropck::check_drops;
use crate::hir::{lower_program, HirProgram};
use crate::ir::{lower_to_ir, IrProgram};
use crate::ownership::check_ownership;
use crate::package::NativeLinkConfig;
use crate::resolve::resolve_program;
use crate::semantics::{check_program, Diagnostic, Target};
use crate::threadck::check_threads;
use crate::typeck::type_check_program;

pub mod cranelift;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildOutput {
    pub object_path: PathBuf,
    pub executable_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildOptions {
    pub target: Target,
    pub native: NativeLinkConfig,
    pub linker: LinkerConfig,
}

impl Default for BuildOptions {
    fn default() -> Self {
        Self {
            target: Target::host(),
            native: NativeLinkConfig::default(),
            linker: LinkerConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LinkerConfig {
    pub program: Option<String>,
    pub args: Vec<String>,
}

pub fn build_program(program: &Program, output: &Path) -> Result<BuildOutput, Vec<Diagnostic>> {
    build_program_with_options(program, output, &BuildOptions::default())
}

pub fn build_program_with_options(
    program: &Program,
    output: &Path,
    options: &BuildOptions,
) -> Result<BuildOutput, Vec<Diagnostic>> {
    let hir = checked_hir(program, &options.target)?;
    let drops = check_drops(&hir)?;
    check_threads(&hir)?;
    let ir = lower_to_ir(&hir, &drops);
    build_ir(&ir, output, options)
}

fn checked_hir(program: &Program, target: &Target) -> Result<HirProgram, Vec<Diagnostic>> {
    check_program(program, target)?;
    let hir = lower_program(program, target)?;
    resolve_program(&hir)?;
    type_check_program(&hir)?;
    check_ownership(&hir)?;
    check_borrows(&hir)?;
    Ok(hir)
}

fn build_ir(
    ir: &IrProgram,
    output: &Path,
    options: &BuildOptions,
) -> Result<BuildOutput, Vec<Diagnostic>> {
    let object_path = output.with_extension("o");
    if let Some(parent) = object_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| diagnostic(format!("failed to create output directory: {err}")))?;
    }
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| diagnostic(format!("failed to create output directory: {err}")))?;
    }
    cranelift::emit_object(ir, &object_path, &options.target)?;
    link_object(
        &object_path,
        output,
        &options.native,
        &options.target,
        &options.linker,
    )?;
    Ok(BuildOutput {
        object_path,
        executable_path: output.to_path_buf(),
    })
}

fn link_object(
    object_path: &Path,
    output: &Path,
    native: &NativeLinkConfig,
    target: &Target,
    linker: &LinkerConfig,
) -> Result<(), Vec<Diagnostic>> {
    validate_native_inputs(native)?;
    if !target.is_host() && linker.program.is_none() {
        return Err(diagnostic(format!(
            "cross-target linking requires --linker; host is {}, target is {}",
            Target::host().triple(),
            target.triple()
        )));
    }
    let mut command = linker_command(target, linker);
    add_target_linker_flags(&mut command, target, linker);
    command.arg(object_path);
    for object in &native.objects {
        command.arg(object);
    }
    for library in &native.static_libraries {
        command.arg(library);
    }
    for path in &native.library_paths {
        command.arg("-L").arg(path);
    }
    for library in &native.libraries {
        command.arg(format!("-l{library}"));
    }
    for arg in &native.link_args {
        command.arg(arg);
    }
    let status = command
        .arg("-o")
        .arg(output)
        .status()
        .map_err(|err| diagnostic(format!("failed to invoke linker: {err}")))?;

    if status.success() {
        Ok(())
    } else {
        Err(diagnostic(format!("linker exited with status {status}")))
    }
}

fn linker_command(target: &Target, linker: &LinkerConfig) -> Command {
    let mut command = match linker.program.as_deref() {
        Some("zig-cc") => {
            let mut command = Command::new("zig");
            command.arg("cc");
            command
        }
        Some(program) => Command::new(program),
        None => Command::new(target.default_linker()),
    };
    for arg in &linker.args {
        command.arg(arg);
    }
    command
}

fn add_target_linker_flags(command: &mut Command, target: &Target, linker: &LinkerConfig) {
    if linker.program.as_deref() == Some("zig-cc") {
        command.arg("-target").arg(target.zig_triple());
        return;
    }
    if target.has("darwin") && target.has("aarch64") {
        command.arg("-arch").arg("arm64");
    } else if target.has("darwin") && target.has("x86_64") {
        command.arg("-arch").arg("x86_64");
    }
}

fn validate_native_inputs(native: &NativeLinkConfig) -> Result<(), Vec<Diagnostic>> {
    let mut errors = Vec::new();
    for library in &native.static_libraries {
        if !library.is_file() {
            errors.push(Diagnostic {
                message: format!("native static library not found: {}", library.display()),
                location: None,
                code: None,
            });
        }
    }
    for object in &native.objects {
        if !object.is_file() {
            errors.push(Diagnostic {
                message: format!("native object file not found: {}", object.display()),
                location: None,
                code: None,
            });
        }
    }
    for path in &native.library_paths {
        if !path.is_dir() {
            errors.push(Diagnostic {
                message: format!("native library path not found: {}", path.display()),
                location: None,
                code: None,
            });
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn diagnostic(message: String) -> Vec<Diagnostic> {
    vec![Diagnostic {
        message,
        location: None,
        code: None,
    }]
}
