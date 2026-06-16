use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{self, Child, Command};
use std::time::{Duration, Instant};

use mo::ast::*;
use mo::backend::{build_program_with_options, BuildOptions, LinkerConfig};
use mo::borrow::check_borrows;
use mo::dropck::{check_drops, DropReport};
use mo::hir::{lower_program, HirProgram};
use mo::ir::{lower_to_ir, IrProgram};
use mo::ownership::check_ownership;
use mo::package::{
    load_manifest_scripts, load_package_with_metadata, LoadedPackage, NativeLinkConfig,
};
use mo::resolve::resolve_program;
use mo::semantics::{check_program, Target};
use mo::span::line_column;
use mo::threadck::check_threads;
use mo::typeck::type_check_program;
use mo::Lexer;

fn main() {
    let cli = parse_cli();

    if cli.command == "test" {
        if cli.watch {
            watch_tests(Path::new(&cli.path), &cli);
        } else {
            run_tests(Path::new(&cli.path), &cli);
        }
        return;
    }

    if cli.command == "example" {
        run_example(&cli.path, &cli);
        return;
    }

    if cli.command == "smoke" || cli.command == "bench" {
        run_smoke_or_bench(&cli.command, &cli.path, &cli.extra_args);
        return;
    }

    if cli.command == "fmt" {
        run_fmt(Path::new(&cli.path), cli.fmt_check);
        return;
    }

    if cli.command == "exec" {
        run_manifest_script(
            cli.exec_script.as_deref().expect("exec script name"),
            Path::new(&cli.path),
            &cli.extra_args,
        );
        return;
    }

    let path = Path::new(&cli.path);
    if cli.command == "run" && cli.watch {
        watch_run(path, &cli);
        return;
    }
    let source = read_source(path);

    let tokens = Lexer::new(&source).tokenize().unwrap_or_else(|err| {
        let (line, column) = line_column(&source, err.span.start);
        eprintln!(
            "{}:{line}:{column}: lex error [MO0001]: {}",
            path.display(),
            err.message
        );
        process::exit(1);
    });

    if cli.command == "tokens" {
        for token in tokens {
            println!("{:?} {}..{}", token.kind, token.span.start, token.span.end);
        }
        return;
    }

    let loaded = load_package_or_exit(path, &cli.target);

    if cli.command == "ast" {
        println!("{:#?}", loaded.program);
    } else if cli.command == "build" {
        let analysis = analyze_or_exit(&loaded.program, &cli.target);
        emit_debug_dumps(&analysis, &cli);
        let output = cli.output.clone().unwrap_or_else(|| "a.out".to_string());
        let metrics = build_or_exit(&loaded.program, &loaded.native, &cli, Path::new(&output));
        println!("{}", metrics.executable_path.display());
        emit_metrics(&metrics, cli.metrics, cli.metrics_json.as_deref());
    } else if cli.command == "run" {
        let analysis = analyze_or_exit(&loaded.program, &cli.target);
        emit_debug_dumps(&analysis, &cli);
        run_program(
            &loaded.program,
            &loaded.native,
            &cli,
            path,
            cli.metrics,
            cli.metrics_json.as_deref(),
            cli.memory_report,
        );
    } else {
        let analysis = analyze_or_exit(&loaded.program, &cli.target);
        emit_debug_dumps(&analysis, &cli);
        println!("ok");
    }
}

#[derive(Debug, Clone)]
struct CliOptions {
    command: String,
    path: String,
    output: Option<String>,
    metrics: bool,
    metrics_json: Option<String>,
    fmt_check: bool,
    test_options: TestOptions,
    emit_hir: bool,
    emit_ir: bool,
    dump_drops: bool,
    dump_ownership: bool,
    memory_report: bool,
    watch: bool,
    debounce_ms: u64,
    extra_args: Vec<String>,
    exec_script: Option<String>,
    target: Target,
    linker: LinkerConfig,
}

#[derive(Debug, Clone, Default)]
struct TestOptions {
    filter: Option<String>,
    verbose: bool,
    json: bool,
    keep_build: bool,
    list: bool,
    timeout_seconds: Option<u64>,
}

#[derive(Debug, Clone)]
struct BuildMetrics {
    executable_path: PathBuf,
    object_path: PathBuf,
    build_wall_seconds: f64,
    object_size_bytes: u64,
    executable_size_bytes: u64,
    compiler_max_rss_bytes: Option<i64>,
    run_wall_seconds: Option<f64>,
    run_exit_code: Option<i32>,
    run_max_rss_bytes: Option<i64>,
}

fn parse_cli() -> CliOptions {
    let mut args = env::args().skip(1);
    let Some(command_or_path) = args.next() else {
        usage_and_exit();
    };

    let mut exec_script = None;
    let (command, path) = match command_or_path.as_str() {
        "check" | "tokens" | "ast" | "test" | "build" | "run" | "fmt" | "example" | "smoke"
        | "bench" => {
            let Some(path) = args.next() else {
                eprintln!("usage: mo {} <file-or-dir-or-name>", command_or_path);
                process::exit(2);
            };
            (command_or_path, path)
        }
        "exec" => {
            let Some(script) = args.next() else {
                eprintln!("usage: mo exec <script> [project-dir] [-- script args]");
                process::exit(2);
            };
            exec_script = Some(script);
            let path = args.next().unwrap_or_else(|| ".".to_string());
            ("exec".to_string(), path)
        }
        path => ("check".to_string(), path.to_string()),
    };

    let mut output = None;
    let mut metrics = false;
    let mut metrics_json = None;
    let mut fmt_check = false;
    let mut test_options = TestOptions::default();
    let mut emit_hir = false;
    let mut emit_ir = false;
    let mut dump_drops = false;
    let mut dump_ownership = false;
    let mut memory_report = false;
    let mut watch = false;
    let mut debounce_ms = 250;
    let mut extra_args = Vec::new();
    let mut target = Target::host();
    let mut linker = LinkerConfig::default();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-o" if command == "build" => {
                output = args.next();
                if output.is_none() {
                    eprintln!("usage: mo build <file> -o <output>");
                    process::exit(2);
                }
            }
            "--metrics" if command == "build" || command == "run" => metrics = true,
            "--metrics-json" if command == "build" || command == "run" => {
                metrics_json = args.next();
                if metrics_json.is_none() {
                    eprintln!("usage: mo {command} <file> --metrics-json <path>");
                    process::exit(2);
                }
            }
            "--check" if command == "fmt" => fmt_check = true,
            "--filter" if command == "test" => {
                test_options.filter = args.next();
                if test_options.filter.is_none() {
                    eprintln!("usage: mo test <file-or-dir> --filter <text>");
                    process::exit(2);
                }
            }
            "--verbose" if command == "test" => test_options.verbose = true,
            "--json" if command == "test" => test_options.json = true,
            "--keep-build" if command == "test" => test_options.keep_build = true,
            "--list" if command == "test" => test_options.list = true,
            "--timeout" if command == "test" => {
                let Some(value) = args.next() else {
                    eprintln!("usage: mo test <file-or-dir> --timeout <seconds>");
                    process::exit(2);
                };
                test_options.timeout_seconds = Some(value.parse().unwrap_or_else(|_| {
                    eprintln!("invalid --timeout value `{value}`; expected seconds");
                    process::exit(2);
                }));
            }
            "--run-native" if command == "test" => {}
            "--emit-hir" if command == "check" || command == "build" || command == "run" => {
                emit_hir = true
            }
            "--emit-ir" if command == "check" || command == "build" || command == "run" => {
                emit_ir = true
            }
            "--dump-drops" if command == "check" || command == "build" || command == "run" => {
                dump_drops = true
            }
            "--dump-ownership" if command == "check" || command == "build" || command == "run" => {
                dump_ownership = true
            }
            "--memory-report" if command == "run" => memory_report = true,
            "--watch" if command == "run" || command == "test" => watch = true,
            "--debounce-ms" if command == "run" || command == "test" => {
                let Some(value) = args.next() else {
                    eprintln!("usage: mo {command} <file-or-dir> --debounce-ms <ms>");
                    process::exit(2);
                };
                debounce_ms = value.parse().unwrap_or_else(|_| {
                    eprintln!("invalid --debounce-ms value `{value}`; expected milliseconds");
                    process::exit(2);
                });
            }
            "--target"
                if command == "check"
                    || command == "build"
                    || command == "run"
                    || command == "test" =>
            {
                let Some(value) = args.next() else {
                    eprintln!("usage: mo {command} <file-or-dir> --target <target>");
                    process::exit(2);
                };
                target = Target::parse(&value).unwrap_or_else(|err| {
                    eprintln!("{err}");
                    process::exit(2);
                });
            }
            "--linker" if command == "build" || command == "run" || command == "test" => {
                linker.program = args.next();
                if linker.program.is_none() {
                    eprintln!("usage: mo {command} <file-or-dir> --linker <program>");
                    process::exit(2);
                }
            }
            "--linker-arg" if command == "build" || command == "run" || command == "test" => {
                let Some(value) = args.next() else {
                    eprintln!("usage: mo {command} <file-or-dir> --linker-arg <arg>");
                    process::exit(2);
                };
                linker.args.push(value);
            }
            "--" if command == "example"
                || command == "smoke"
                || command == "bench"
                || command == "exec" =>
            {
                extra_args.extend(args);
                break;
            }
            _ if command == "example" || command == "smoke" || command == "bench" => {
                extra_args.push(arg);
            }
            _ if command == "exec" => {
                eprintln!("unknown option for mo exec: {arg}");
                process::exit(2);
            }
            _ => {
                eprintln!("unknown option for mo {command}: {arg}");
                process::exit(2);
            }
        }
    }

    CliOptions {
        command,
        path,
        output,
        metrics,
        metrics_json,
        fmt_check,
        test_options,
        emit_hir,
        emit_ir,
        dump_drops,
        dump_ownership,
        memory_report,
        watch,
        debounce_ms,
        extra_args,
        exec_script,
        target,
        linker,
    }
}

fn usage_and_exit() -> ! {
    eprintln!(
        "usage: mo <check|tokens|ast|test|build|run|fmt|example|smoke|bench> <file-or-dir-or-name> [options]\n\
         usage: mo exec <script> [project-dir] [-- script args]\n\
         options:\n\
           build <file> -o <output> [--target <target>] [--linker <program>] [--linker-arg <arg>] [--metrics] [--metrics-json <path>] [--emit-hir] [--emit-ir] [--dump-drops] [--dump-ownership]\n\
           run <file> [--target <target>] [--linker <program>] [--linker-arg <arg>] [--watch] [--debounce-ms <ms>] [--metrics] [--metrics-json <path>] [--memory-report] [--emit-hir] [--emit-ir] [--dump-drops] [--dump-ownership]\n\
           test <file-or-dir> [--target <target>] [--linker <program>] [--linker-arg <arg>] [--watch] [--debounce-ms <ms>] [--filter <text>] [--list] [--verbose] [--json] [--keep-build] [--timeout <seconds>] [--run-native]\n\
           fmt <file-or-dir> [--check]\n\
           exec <script> [project-dir] [-- script args]\n\
           example <name-or-path> [-- extra args]\n\
           smoke pokemon-server [script args]\n\
           bench pokemon-server [script args]"
    );
    process::exit(2);
}

fn run_manifest_script(script: &str, path: &Path, extra_args: &[String]) {
    let loaded = load_manifest_scripts(path).unwrap_or_else(|errors| {
        for error in errors {
            eprintln!("{}", error.message);
        }
        process::exit(1);
    });
    let Some(command) = loaded.scripts.get(script) else {
        let mut names = loaded.scripts.keys().cloned().collect::<Vec<_>>();
        names.sort();
        if names.is_empty() {
            eprintln!(
                "manifest `{}` does not define any scripts",
                loaded.manifest.display()
            );
        } else {
            eprintln!(
                "script `{script}` not found in `{}`; available scripts: {}",
                loaded.manifest.display(),
                names.join(", ")
            );
        }
        process::exit(1);
    };

    let command = append_script_args(command, extra_args);
    println!("$ {command}");
    let status = Command::new("sh")
        .arg("-c")
        .arg(&command)
        .current_dir(&loaded.manifest_dir)
        .status()
        .unwrap_or_else(|err| {
            eprintln!("failed to execute script `{script}`: {err}");
            process::exit(1);
        });

    if !status.success() {
        process::exit(status.code().unwrap_or(1));
    }
}

fn append_script_args(command: &str, extra_args: &[String]) -> String {
    if extra_args.is_empty() {
        return command.to_string();
    }
    let mut output = command.to_string();
    for arg in extra_args {
        output.push(' ');
        output.push_str(&shell_quote(arg));
    }
    output
}

fn shell_quote(value: &str) -> String {
    if value.is_empty() {
        return "''".to_string();
    }
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | '/' | ':' | '='))
    {
        return value.to_string();
    }
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn watch_run(entry: &Path, cli: &CliOptions) {
    eprintln!("watching {} (poll {}ms)", entry.display(), cli.debounce_ms);
    let mut snapshot = watch_snapshot(entry);
    let mut child = rebuild_and_spawn(entry, cli);
    loop {
        if let Some(running) = child.as_mut() {
            if running.try_wait().ok().flatten().is_some() {
                child = None;
            }
        }
        std::thread::sleep(Duration::from_millis(cli.debounce_ms));
        let next = watch_snapshot(entry);
        if next != snapshot {
            snapshot = next;
            eprintln!("change detected; rebuilding {}", entry.display());
            stop_child(&mut child);
            child = rebuild_and_spawn(entry, cli);
        }
    }
}

fn rebuild_and_spawn(entry: &Path, cli: &CliOptions) -> Option<Child> {
    let loaded = match load_package_with_metadata(entry, &cli.target) {
        Ok(loaded) => loaded,
        Err(errors) => {
            for error in errors {
                eprintln!("{}", error.message);
            }
            return None;
        }
    };
    let analysis = analyze_program(&loaded.program, &cli.target)
        .map_err(|errors| {
            for (label, diagnostic) in errors {
                print_analysis_error(label, &diagnostic);
            }
        })
        .ok()?;
    emit_debug_dumps(&analysis, cli);
    let executable_path = env::temp_dir().join(format!(
        "mo_watch_{}_{}",
        process::id(),
        entry
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("main")
    ));
    match build_program_with_options(
        &loaded.program,
        &executable_path,
        &BuildOptions {
            target: cli.target.clone(),
            native: loaded.native,
            linker: cli.linker.clone(),
        },
    ) {
        Ok(_) => {}
        Err(errors) => {
            for error in errors {
                eprintln!("build error: {}", error.message);
            }
            return None;
        }
    }
    eprintln!("running {}", executable_path.display());
    Command::new(&executable_path)
        .spawn()
        .map(Some)
        .unwrap_or_else(|err| {
            eprintln!("run error: {err}");
            None
        })
}

fn stop_child(child: &mut Option<Child>) {
    if let Some(mut running) = child.take() {
        let _ = running.kill();
        let _ = running.wait();
    }
}

fn watch_tests(path: &Path, cli: &CliOptions) {
    eprintln!("watching {} (poll {}ms)", path.display(), cli.debounce_ms);
    let mut snapshot = watch_snapshot(path);
    run_test_command_once(path, cli);
    loop {
        std::thread::sleep(Duration::from_millis(cli.debounce_ms));
        let next = watch_snapshot(path);
        if next != snapshot {
            snapshot = next;
            eprintln!("change detected; rerunning tests");
            run_test_command_once(path, cli);
        }
    }
}

fn run_test_command_once(path: &Path, cli: &CliOptions) {
    let exe = env::current_exe().unwrap_or_else(|err| {
        eprintln!("failed to resolve current executable: {err}");
        process::exit(1);
    });
    let mut command = Command::new(exe);
    command.arg("test").arg(path);
    if let Some(filter) = &cli.test_options.filter {
        command.arg("--filter").arg(filter);
    }
    if cli.test_options.verbose {
        command.arg("--verbose");
    }
    if cli.test_options.json {
        command.arg("--json");
    }
    if cli.test_options.keep_build {
        command.arg("--keep-build");
    }
    if cli.test_options.list {
        command.arg("--list");
    }
    if let Some(timeout) = cli.test_options.timeout_seconds {
        command.arg("--timeout").arg(timeout.to_string());
    }
    let _ = command.status();
}

fn watch_snapshot(entry: &Path) -> BTreeMap<PathBuf, Option<std::time::SystemTime>> {
    let mut files = Vec::new();
    push_watch_files(entry, &mut files);
    for dir in ["core", "std", "lib"] {
        let path = Path::new(dir);
        if path.exists() {
            push_watch_files(path, &mut files);
        }
    }
    files.sort();
    files.dedup();
    files
        .into_iter()
        .map(|file| {
            let modified = fs::metadata(&file).and_then(|meta| meta.modified()).ok();
            (file, modified)
        })
        .collect()
}

fn push_watch_files(path: &Path, files: &mut Vec<PathBuf>) {
    if path.is_file() {
        if path.extension().is_some_and(|ext| ext == "mo") {
            files.push(path.to_path_buf());
        }
        return;
    }
    if let Ok(found) = discover_mo_files(path) {
        files.extend(found);
    }
}

fn run_program(
    program: &Program,
    native: &NativeLinkConfig,
    cli: &CliOptions,
    source_path: &Path,
    metrics: bool,
    metrics_json: Option<&str>,
    memory_report: bool,
) {
    let executable_path = env::temp_dir().join(format!(
        "mo_run_{}_{}",
        process::id(),
        source_path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("main")
    ));
    let mut build_metrics = build_or_exit(program, native, cli, &executable_path);

    let started = Instant::now();
    let run_result = run_child_with_usage(&executable_path);
    build_metrics.run_wall_seconds = Some(started.elapsed().as_secs_f64());
    build_metrics.run_exit_code = run_result.exit_code;
    build_metrics.run_max_rss_bytes = run_result.max_rss_bytes;
    emit_metrics(&build_metrics, metrics, metrics_json);
    if memory_report {
        emit_memory_report_hint(&build_metrics);
    }
    process::exit(run_result.exit_code.unwrap_or(1));
}

#[derive(Debug, Clone)]
struct Analysis {
    hir: HirProgram,
    drops: DropReport,
    ir: IrProgram,
}

fn analyze_or_exit(program: &Program, target: &Target) -> Analysis {
    match analyze_program(program, target) {
        Ok(analysis) => analysis,
        Err(errors) => {
            for (label, error) in errors {
                print_analysis_error(label, &error);
            }
            process::exit(1);
        }
    }
}

fn analyze_program(
    program: &Program,
    target: &Target,
) -> Result<Analysis, Vec<(&'static str, mo::semantics::Diagnostic)>> {
    if let Err(errors) = check_program(program, &target) {
        return Err(label_errors("semantic", errors));
    }
    let hir = match lower_program(program, &target) {
        Ok(hir) => hir,
        Err(errors) => return Err(label_errors("lowering", errors)),
    };
    if let Err(errors) = resolve_program(&hir) {
        return Err(label_errors("resolve", errors));
    }
    if let Err(errors) = type_check_program(&hir) {
        return Err(label_errors("type", errors));
    }
    if let Err(errors) = check_ownership(&hir) {
        return Err(label_errors("ownership", errors));
    }
    if let Err(errors) = check_borrows(&hir) {
        return Err(label_errors("borrow", errors));
    }
    let drops = match check_drops(&hir) {
        Ok(drops) => drops,
        Err(errors) => return Err(label_errors("drop", errors)),
    };
    if let Err(errors) = check_threads(&hir) {
        return Err(label_errors("thread", errors));
    }
    let ir = lower_to_ir(&hir, &drops);
    Ok(Analysis { hir, drops, ir })
}

fn label_errors(
    label: &'static str,
    errors: Vec<mo::semantics::Diagnostic>,
) -> Vec<(&'static str, mo::semantics::Diagnostic)> {
    errors.into_iter().map(|error| (label, error)).collect()
}

fn print_analysis_error(label: &str, error: &mo::semantics::Diagnostic) {
    let label = if let Some(code) = &error.code {
        format!("{label} error [{code}]")
    } else {
        format!("{label} error")
    };
    if let Some(location) = &error.location {
        eprintln!("{location}: {label}: {}", error.message);
    } else {
        eprintln!("{label}: {}", error.message);
    }
}

fn emit_debug_dumps(analysis: &Analysis, cli: &CliOptions) {
    if cli.emit_hir {
        println!("{:#?}", analysis.hir);
    }
    if cli.dump_drops {
        println!("{:#?}", analysis.drops);
    }
    if cli.dump_ownership {
        println!(
            "ownership: checked {} function/test body item(s)",
            analysis.hir.functions.len() + analysis.hir.tests.len()
        );
    }
    if cli.emit_ir {
        println!("{:#?}", analysis.ir);
    }
}

fn emit_memory_report_hint(metrics: &BuildMetrics) {
    eprintln!("memory report:");
    eprintln!("  runtime counters: expose with core.mem_alloc_count/free_count/live_bytes/high_water_bytes in program-level tests");
    eprintln!(
        "  process max rss bytes: {}",
        option_i64_json(metrics.run_max_rss_bytes)
    );
}

fn build_or_exit(
    program: &Program,
    native: &NativeLinkConfig,
    cli: &CliOptions,
    output: &Path,
) -> BuildMetrics {
    let started = Instant::now();
    let result = match build_program_with_options(
        program,
        output,
        &BuildOptions {
            target: cli.target.clone(),
            native: native.clone(),
            linker: cli.linker.clone(),
        },
    ) {
        Ok(result) => result,
        Err(errors) => {
            for error in errors {
                eprintln!("build error: {}", error.message);
            }
            process::exit(1);
        }
    };
    let build_wall_seconds = started.elapsed().as_secs_f64();
    let object_size_bytes = fs::metadata(&result.object_path)
        .map(|m| m.len())
        .unwrap_or(0);
    let executable_size_bytes = fs::metadata(&result.executable_path)
        .map(|m| m.len())
        .unwrap_or(0);
    BuildMetrics {
        executable_path: result.executable_path,
        object_path: result.object_path,
        build_wall_seconds,
        object_size_bytes,
        executable_size_bytes,
        compiler_max_rss_bytes: self_max_rss_bytes(),
        run_wall_seconds: None,
        run_exit_code: None,
        run_max_rss_bytes: None,
    }
}

fn emit_metrics(metrics: &BuildMetrics, print_metrics: bool, metrics_json: Option<&str>) {
    if print_metrics {
        eprintln!("{}", metrics_json_text(metrics));
    }
    if let Some(path) = metrics_json {
        fs::write(path, metrics_json_text(metrics)).unwrap_or_else(|err| {
            eprintln!("{path}: {err}");
            process::exit(1);
        });
    }
}

fn metrics_json_text(metrics: &BuildMetrics) -> String {
    let run_wall = option_f64_json(metrics.run_wall_seconds);
    let run_exit = option_i32_json(metrics.run_exit_code);
    format!(
        "{{\n  \"executable_path\": \"{}\",\n  \"object_path\": \"{}\",\n  \"build_wall_seconds\": {},\n  \"object_size_bytes\": {},\n  \"executable_size_bytes\": {},\n  \"compiler_max_rss_bytes\": {},\n  \"run_wall_seconds\": {},\n  \"run_exit_code\": {},\n  \"run_max_rss_bytes\": {}\n}}\n",
        json_escape(&metrics.executable_path.display().to_string()),
        json_escape(&metrics.object_path.display().to_string()),
        metrics.build_wall_seconds,
        metrics.object_size_bytes,
        metrics.executable_size_bytes,
        option_i64_json(metrics.compiler_max_rss_bytes),
        run_wall,
        run_exit,
        option_i64_json(metrics.run_max_rss_bytes),
    )
}

fn option_f64_json(value: Option<f64>) -> String {
    value.map_or_else(|| "null".to_string(), |value| value.to_string())
}

fn option_i32_json(value: Option<i32>) -> String {
    value.map_or_else(|| "null".to_string(), |value| value.to_string())
}

fn option_i64_json(value: Option<i64>) -> String {
    value.map_or_else(|| "null".to_string(), |value| value.to_string())
}

#[derive(Debug, Clone, Copy)]
struct RunUsage {
    exit_code: Option<i32>,
    max_rss_bytes: Option<i64>,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct TimeVal {
    tv_sec: i64,
    tv_usec: i32,
    _pad: i32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct RUsage {
    ru_utime: TimeVal,
    ru_stime: TimeVal,
    ru_maxrss: i64,
    ru_ixrss: i64,
    ru_idrss: i64,
    ru_isrss: i64,
    ru_minflt: i64,
    ru_majflt: i64,
    ru_nswap: i64,
    ru_inblock: i64,
    ru_oublock: i64,
    ru_msgsnd: i64,
    ru_msgrcv: i64,
    ru_nsignals: i64,
    ru_nvcsw: i64,
    ru_nivcsw: i64,
}

unsafe extern "C" {
    fn getrusage(who: i32, usage: *mut RUsage) -> i32;
    fn wait4(pid: i32, status: *mut i32, options: i32, usage: *mut RUsage) -> i32;
}

const RUSAGE_SELF: i32 = 0;

fn self_max_rss_bytes() -> Option<i64> {
    let mut usage = RUsage::default();
    let ok = unsafe { getrusage(RUSAGE_SELF, &mut usage) };
    if ok == 0 && usage.ru_maxrss > 0 {
        Some(usage.ru_maxrss)
    } else {
        None
    }
}

fn run_child_with_usage(executable_path: &Path) -> RunUsage {
    let mut child = Command::new(executable_path).spawn().unwrap_or_else(|err| {
        eprintln!("run error: {err}");
        process::exit(1);
    });
    let pid = child.id() as i32;
    let mut status = 0i32;
    let mut usage = RUsage::default();
    let waited = unsafe { wait4(pid, &mut status, 0, &mut usage) };
    if waited < 0 {
        let fallback = child.wait().unwrap_or_else(|err| {
            eprintln!("run error: {err}");
            process::exit(1);
        });
        return RunUsage {
            exit_code: fallback.code(),
            max_rss_bytes: None,
        };
    }
    RunUsage {
        exit_code: wait_exit_code(status),
        max_rss_bytes: if usage.ru_maxrss > 0 {
            Some(usage.ru_maxrss)
        } else {
            None
        },
    }
}

fn wait_exit_code(status: i32) -> Option<i32> {
    if status & 0x7f == 0 {
        Some((status >> 8) & 0xff)
    } else {
        None
    }
}

fn json_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn run_fmt(path: &Path, check: bool) {
    let files = discover_mo_files(path).unwrap_or_else(|err| {
        eprintln!("{}: {err}", path.display());
        process::exit(1);
    });
    if files.is_empty() {
        eprintln!("{}: no .mo files found", path.display());
        process::exit(1);
    }

    let mut changed = Vec::new();
    for file in files {
        let original = read_source(&file);
        let formatted = format_mo_source(&original);
        if formatted != original {
            changed.push(file.clone());
            if !check {
                fs::write(&file, formatted).unwrap_or_else(|err| {
                    eprintln!("{}: {err}", file.display());
                    process::exit(1);
                });
            }
        }
    }

    if check && !changed.is_empty() {
        for file in changed {
            println!("{}: needs formatting", file.display());
        }
        process::exit(1);
    }
}

fn format_mo_source(source: &str) -> String {
    let mut out = String::new();
    let mut indent = 0usize;
    let mut previous_blank = false;

    for raw_line in source.lines() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() {
            if !previous_blank && !out.is_empty() {
                out.push('\n');
            }
            previous_blank = true;
            continue;
        }

        if trimmed.starts_with('}') && indent > 0 {
            indent -= 1;
        }

        for _ in 0..indent {
            out.push_str("    ");
        }
        out.push_str(trimmed);
        out.push('\n');
        previous_blank = false;

        let opens = trimmed.chars().filter(|ch| *ch == '{').count();
        let closes = trimmed.chars().filter(|ch| *ch == '}').count();
        if opens > closes {
            indent += opens - closes;
        } else if closes > opens {
            indent = indent.saturating_sub(closes - opens);
        }
    }

    if out.is_empty() {
        out
    } else if out.ends_with('\n') {
        out
    } else {
        out.push('\n');
        out
    }
}

fn run_tests(path: &Path, cli: &CliOptions) {
    let options = &cli.test_options;
    let files = discover_mo_files(path).unwrap_or_else(|err| {
        eprintln!("{}: {err}", path.display());
        process::exit(1);
    });

    if files.is_empty() {
        eprintln!("{}: no .mo files found", path.display());
        process::exit(1);
    }

    let mut total_tests = 0usize;
    let mut failed_tests = 0usize;

    for file in files {
        let loaded = load_package_or_exit(&file, &cli.target);
        let tests = collect_tests(&loaded.program)
            .into_iter()
            .filter(|test| test_matches_filter(&file, &test.name, options.filter.as_deref()))
            .collect::<Vec<_>>();
        total_tests += tests.len();

        for test in tests {
            if options.list {
                if options.json {
                    println!(
                        "{{\"file\":\"{}\",\"test\":\"{}\"}}",
                        json_escape(&file.display().to_string()),
                        json_escape(&test.name)
                    );
                } else {
                    println!("{}: test {}", file.display(), test.name);
                }
                continue;
            }

            let test_program = program_for_test(&loaded.program, &test);
            let executable_path = test_executable_path(&file, &test.name);
            match build_program_with_options(
                &test_program,
                &executable_path,
                &BuildOptions {
                    target: cli.target.clone(),
                    native: loaded.native.clone(),
                    linker: cli.linker.clone(),
                },
            ) {
                Ok(_) => {
                    if options.verbose {
                        eprintln!(
                            "{}: test {}: built {}",
                            file.display(),
                            test.name,
                            executable_path.display()
                        );
                    }
                }
                Err(errors) => {
                    failed_tests += 1;
                    for error in errors {
                        emit_test_result(
                            options,
                            &file,
                            &test.name,
                            "build_error",
                            None,
                            &error.message,
                        );
                    }
                    continue;
                }
            }

            let test_run = run_test_child(&executable_path, options.timeout_seconds)
                .unwrap_or_else(|err| {
                    eprintln!("{}: test {}: run error: {err}", file.display(), test.name);
                    process::exit(1);
                });
            if !options.keep_build {
                let _ = fs::remove_file(&executable_path);
                let _ = fs::remove_file(executable_path.with_extension("o"));
            }
            if test_run.timed_out {
                failed_tests += 1;
                emit_test_result(options, &file, &test.name, "timeout", None, "timed out");
            } else {
                match test_run.exit_code {
                    Some(0) => emit_test_result(options, &file, &test.name, "ok", Some(0), ""),
                    Some(code) => {
                        failed_tests += 1;
                        emit_test_result(options, &file, &test.name, "failed", Some(code), "");
                    }
                    None => {
                        failed_tests += 1;
                        emit_test_result(
                            options,
                            &file,
                            &test.name,
                            "failed",
                            None,
                            "terminated by signal",
                        );
                    }
                }
            }
        }
    }

    let passed_tests = total_tests.saturating_sub(failed_tests);
    if options.json {
        println!(
            "{{\"summary\":{{\"passed\":{},\"failed\":{},\"total\":{}}}}}",
            passed_tests, failed_tests, total_tests
        );
    } else if !options.list {
        println!("{passed_tests}/{total_tests} test(s) passed");
    }
    if failed_tests > 0 {
        process::exit(1);
    }
}

#[derive(Debug, Clone, Copy)]
struct TestRun {
    exit_code: Option<i32>,
    timed_out: bool,
}

fn run_test_child(
    executable_path: &Path,
    timeout_seconds: Option<u64>,
) -> Result<TestRun, std::io::Error> {
    let mut child = Command::new(executable_path).spawn()?;
    let Some(timeout_seconds) = timeout_seconds else {
        let status = child.wait()?;
        return Ok(TestRun {
            exit_code: status.code(),
            timed_out: false,
        });
    };
    let deadline = Instant::now() + Duration::from_secs(timeout_seconds);
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(TestRun {
                exit_code: status.code(),
                timed_out: false,
            });
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(TestRun {
                exit_code: None,
                timed_out: true,
            });
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn test_matches_filter(file: &Path, name: &str, filter: Option<&str>) -> bool {
    let Some(filter) = filter else {
        return true;
    };
    name.contains(filter) || file.display().to_string().contains(filter)
}

fn emit_test_result(
    options: &TestOptions,
    file: &Path,
    name: &str,
    status: &str,
    code: Option<i32>,
    message: &str,
) {
    if options.json {
        println!(
            "{{\"file\":\"{}\",\"test\":\"{}\",\"status\":\"{}\",\"exit_code\":{},\"message\":\"{}\"}}",
            json_escape(&file.display().to_string()),
            json_escape(name),
            json_escape(status),
            option_i32_json(code),
            json_escape(message)
        );
    } else if status == "ok" {
        println!("{}: test {}: ok", file.display(), name);
    } else if let Some(code) = code {
        println!(
            "{}: test {}: {status} with exit code {code}",
            file.display(),
            name
        );
    } else {
        println!("{}: test {}: {status}: {message}", file.display(), name);
    }
}

fn test_executable_path(file: &Path, test_name: &str) -> PathBuf {
    env::temp_dir().join(format!(
        "mo_test_{}_{}_{}",
        process::id(),
        file.file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("test"),
        sanitize_test_name(test_name)
    ))
}

fn program_for_test(program: &Program, test: &TestItem) -> Program {
    let mut items = Vec::new();
    for item in &program.items {
        match item {
            Item::Test(_) => {}
            Item::Function(function) if function.name == "main" => {}
            _ => items.push(item.clone()),
        }
    }
    items.push(Item::Module(mo::ast::Path {
        segments: vec!["test".to_string(), sanitize_test_name(&test.name)],
    }));
    items.push(Item::Function(FunctionItem {
        span: mo::span::Span::new(0, 0),
        source_location: None,
        public: false,
        is_async: false,
        is_unsafe: false,
        name: "main".to_string(),
        generics: None,
        params: Vec::new(),
        return_type: Some("Int".to_string()),
        return_type_expr: Some(TypeExpr::Path(vec!["Int".to_string()])),
        body: Some(test.body.clone()),
    }));
    Program { items }
}

fn sanitize_test_name(name: &str) -> String {
    let sanitized = name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    if sanitized.is_empty() {
        "test".to_string()
    } else {
        sanitized
    }
}

fn discover_mo_files(path: &Path) -> Result<Vec<PathBuf>, std::io::Error> {
    let mut files = Vec::new();

    if path.is_file() {
        if path.extension().is_some_and(|ext| ext == "mo") {
            files.push(path.to_path_buf());
        }
        return Ok(files);
    }

    discover_mo_files_in_dir(path, &mut files)?;
    files.sort();
    Ok(files)
}

fn discover_mo_files_in_dir(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), std::io::Error> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            discover_mo_files_in_dir(&path, files)?;
        } else if path.extension().is_some_and(|ext| ext == "mo") {
            files.push(path);
        }
    }
    Ok(())
}

fn read_source(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|err| {
        eprintln!("{}: {err}", path.display());
        process::exit(1);
    })
}

fn load_package_or_exit(path: &Path, target: &Target) -> LoadedPackage {
    load_package_with_metadata(path, target).unwrap_or_else(|errors| {
        for error in errors {
            eprintln!("{}", error.message);
        }
        process::exit(1);
    })
}

fn collect_tests(program: &Program) -> Vec<TestItem> {
    let mut tests = Vec::new();
    collect_tests_from_items(&program.items, &mut tests);
    tests
}

fn collect_tests_from_items(items: &[Item], tests: &mut Vec<TestItem>) {
    for item in items {
        match item {
            Item::Test(test) => tests.push(test.clone()),
            Item::Directive(directive) => collect_tests_from_items(&directive.items, tests),
            _ => {}
        }
    }
}

fn run_example(name_or_path: &str, cli: &CliOptions) {
    let path = example_path(name_or_path);
    let loaded = load_package_or_exit(&path, &cli.target);
    let executable_path = env::temp_dir().join(format!(
        "mo_example_{}_{}",
        process::id(),
        path.file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("example")
    ));
    build_or_exit(&loaded.program, &loaded.native, cli, &executable_path);
    let status = Command::new(&executable_path)
        .args(&cli.extra_args)
        .status()
        .unwrap_or_else(|err| {
            eprintln!("{}: run error: {err}", executable_path.display());
            process::exit(1);
        });
    let _ = fs::remove_file(&executable_path);
    let _ = fs::remove_file(executable_path.with_extension("o"));
    process::exit(status.code().unwrap_or(1));
}

fn example_path(name_or_path: &str) -> PathBuf {
    let direct = PathBuf::from(name_or_path);
    if direct.exists() {
        return direct;
    }
    let with_ext = if name_or_path.ends_with(".mo") {
        name_or_path.to_string()
    } else {
        format!("{name_or_path}.mo")
    };
    for prefix in ["examples/compile", "examples/demo"] {
        let candidate = Path::new(prefix).join(&with_ext);
        if candidate.exists() {
            return candidate;
        }
    }
    eprintln!("unknown example `{name_or_path}`");
    process::exit(1);
}

fn run_smoke_or_bench(command: &str, target: &str, args: &[String]) {
    if target != "pokemon-server" {
        eprintln!("mo {command}: unknown target `{target}`");
        process::exit(2);
    }
    let script = Path::new("scripts/load_pokemon_server.py");
    if !script.exists() {
        eprintln!("{}: missing script", script.display());
        process::exit(1);
    }
    if command == "smoke" {
        run_pokemon_server_script(script, default_smoke_args(args));
    } else {
        run_pokemon_server_script(script, args.to_vec());
    }
}

fn default_smoke_args(args: &[String]) -> Vec<String> {
    if !args.is_empty() {
        return args.to_vec();
    }
    vec![
        "--total".to_string(),
        "24".to_string(),
        "--concurrency".to_string(),
        "4".to_string(),
        "--label".to_string(),
        "smoke".to_string(),
    ]
}

fn run_pokemon_server_script(script: &Path, args: Vec<String>) {
    let status = Command::new("python3")
        .arg(script)
        .args(args)
        .status()
        .unwrap_or_else(|err| {
            eprintln!("{}: {err}", script.display());
            process::exit(1);
        });
    process::exit(status.code().unwrap_or(1));
}
