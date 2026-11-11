// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Command dispatch, reporting and exit codes (BLUE31 §2.3, §2.8).
//!
//! The flow is the same for every command: resolve the input, collect [`Diagnostic`]s, print them
//! once, and answer with an exit code derived from what was printed. Keeping the reporting in one
//! place is what makes `--json` and the text form impossible to disagree.

use crate::bundle::{self, Bundle};
use crate::cli::{self, Command, Options};
use crate::diagnostics::{Diagnostic, Severity, IO_FAILED};
use crate::forward::{self, Generated};
use rust_widgets::designer::{is_generated, ArtifactPaths, TargetProfile, GENERATED_MARKER};
use std::path::Path;
use std::process::ExitCode;

/// The command line, decoded to UTF-8, or the argument that could not be.
///
/// `std::env::args()` panics on an argument that is not valid Unicode, so a file name carrying an
/// unusual byte turned a tool that promises structured diagnostics and exit code 2 into a panic
/// (DM6-04). `args_os()` hands the raw value over, and this reports the position instead.
fn utf8_args() -> Result<Vec<String>, (usize, std::ffi::OsString)> {
    std::env::args_os()
        .enumerate()
        .skip(1)
        .map(|(index, arg)| arg.into_string().map_err(|raw| (index, raw)))
        .collect()
}

/// Entry point.
pub fn run() -> ExitCode {
    let args = match utf8_args() {
        Ok(args) => args,
        Err((index, raw)) => {
            // The existing I/O-class diagnostic and the existing exit code: an argument the tool
            // cannot decode is an input problem, not a different kind of failure (DM6-04).
            eprintln!(
                "{}",
                Diagnostic::file(
                    IO_FAILED,
                    Severity::Error,
                    "<argv>",
                    format!("argument {index} is not valid UTF-8: {}", raw.to_string_lossy()),
                )
                .to_text()
            );
            return ExitCode::from(2);
        }
    };
    let options = match cli::parse(args.into_iter()) {
        Ok(options) => options,
        Err(error) => {
            eprintln!("rwui: {error}\n");
            eprintln!("{}", cli::USAGE);
            return ExitCode::from(2);
        }
    };
    if matches!(options.command, Command::Help) {
        println!("{}", cli::USAGE);
        return ExitCode::SUCCESS;
    }

    let json = options.json;
    let (diagnostics, io_failure) = execute(&options);
    report(&diagnostics, json);
    if io_failure {
        return ExitCode::from(2);
    }
    if diagnostics.iter().any(|diagnostic| diagnostic.severity == Severity::Error) {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

/// Runs the command, returning its diagnostics and whether the tool itself could not run.
fn execute(options: &Options) -> (Vec<Diagnostic>, bool) {
    match &options.command {
        Command::Check { input } => run_check(input, options.strict),
        Command::Json2Rust { input, overrides, targets, check } => {
            run_json2rust(input, overrides, targets, *check, options.strict, options.json)
        }
        Command::Fmt { input, output } => (run_fmt(input, output.as_deref()), false),
        Command::Create { name, profile, output_root, rust_widgets_path } => (
            run_create(name, *profile, output_root, rust_widgets_path.as_deref(), options.json),
            false,
        ),
        Command::Help => (Vec::new(), false),
    }
}

/// `create`: scaffold a project and report every file written.
fn run_create(
    name: &str,
    profile: crate::create::ProjectProfile,
    output_root: &Path,
    rust_widgets_path: Option<&Path>,
    json: bool,
) -> Vec<Diagnostic> {
    let request = crate::create::CreateRequest {
        name: name.to_string(),
        profile,
        output_root: output_root.to_path_buf(),
        rust_widgets_path: rust_widgets_path.map(Path::to_path_buf),
    };
    match crate::create::create(&request) {
        Ok(files) => {
            for file in files {
                progress(json, format!("created {} ({} bytes)", file.path.display(), file.bytes));
            }
            Vec::new()
        }
        Err(error) => vec![io_diagnostic(&output_root.join(name), error)],
    }
}

fn run_check(input: &Path, strict: bool) -> (Vec<Diagnostic>, bool) {
    let mut diagnostics = Vec::new();
    let bundle = match bundle::load(input, &bundle::Overrides::default()) {
        Ok(bundle) => bundle,
        Err(error) => return (vec![io_diagnostic(input, error)], true),
    };
    let text = match std::fs::read_to_string(&bundle.document) {
        Ok(text) => text,
        Err(error) => return (vec![io_diagnostic(&bundle.document, error)], true),
    };
    let project = match forward::parse_document(&text, &bundle.document) {
        Ok(project) => project,
        Err(diagnostics) => return (diagnostics, false),
    };
    let size = forward::solve_size(&project, bundle.manifest.width, bundle.manifest.height);
    let targets = match requested_targets(&[], &bundle, &mut diagnostics) {
        Ok(targets) => targets,
        Err(()) => return (diagnostics, false),
    };
    let outcome = forward::generate_targets(
        &text,
        &project,
        &bundle.document,
        size,
        function_name(&bundle),
        &targets,
        strict,
    );
    diagnostics.extend(outcome.diagnostics);
    if bundle.has_stylesheets() {
        diagnostics.extend(forward::stylesheet_gap(&bundle.stylesheets, strict));
    }
    (diagnostics, false)
}

fn run_json2rust(
    input: &Path,
    overrides: &bundle::Overrides,
    cli_targets: &[TargetProfile],
    check: bool,
    strict: bool,
    json: bool,
) -> (Vec<Diagnostic>, bool) {
    let mut diagnostics = Vec::new();
    let bundle = match bundle::load(input, overrides) {
        Ok(bundle) => bundle,
        Err(error) => return (vec![io_diagnostic(input, error)], true),
    };
    let text = match std::fs::read_to_string(&bundle.document) {
        Ok(text) => text,
        Err(error) => return (vec![io_diagnostic(&bundle.document, error)], true),
    };
    let project = match forward::parse_document(&text, &bundle.document) {
        Ok(project) => project,
        Err(diagnostics) => return (diagnostics, false),
    };
    let size = forward::solve_size(&project, bundle.manifest.width, bundle.manifest.height);
    let targets = match requested_targets(cli_targets, &bundle, &mut diagnostics) {
        Ok(targets) => targets,
        Err(()) => return (diagnostics, false),
    };
    let outcome = forward::generate_targets(
        &text,
        &project,
        &bundle.document,
        size,
        function_name(&bundle),
        &targets,
        strict,
    );
    diagnostics.extend(outcome.diagnostics);
    if bundle.has_stylesheets() {
        diagnostics.extend(forward::stylesheet_gap(&bundle.stylesheets, strict));
    }

    let root = bundle.output_root(overrides);
    if check {
        match forward::check_committed(&outcome.generated, &root) {
            Ok(drift) => diagnostics.extend(drift),
            Err(error) => {
                diagnostics.push(io_diagnostic(&root, error.to_string()));
                return (diagnostics, true);
            }
        }
    } else if let Err(error) = write_all(&outcome.generated, &root, json) {
        diagnostics.push(io_diagnostic(&root, error));
        return (diagnostics, true);
    }
    (diagnostics, false)
}

/// `fmt`: re-serialise the document so key order and indentation are stable.
///
/// `serde_json`'s default map is ordered by key, so this changes no value and no key — only the
/// presentation. A document that does not parse is reported, not rewritten.
fn run_fmt(input: &Path, output: Option<&Path>) -> Vec<Diagnostic> {
    let text = match std::fs::read_to_string(input) {
        Ok(text) => text,
        Err(error) => return vec![io_diagnostic(input, error)],
    };
    let file = input.display().to_string();
    let value: serde_json::Value = match serde_json::from_str(&text) {
        Ok(value) => value,
        Err(error) => {
            return vec![Diagnostic::at(
                crate::diagnostics::JSON_PARSE_FAILED,
                Severity::Error,
                &file,
                error.line() as u32,
                error.column() as u32,
                error.to_string(),
            )]
        }
    };
    let pretty = match serde_json::to_string_pretty(&value) {
        Ok(pretty) => format!("{pretty}\n"),
        Err(error) => {
            return vec![Diagnostic::file(
                crate::diagnostics::JSON_PARSE_FAILED,
                Severity::Error,
                &file,
                error.to_string(),
            )]
        }
    };
    match output {
        Some(path) => match std::fs::write(path, pretty) {
            Ok(()) => Vec::new(),
            Err(error) => vec![io_diagnostic(path, error)],
        },
        None => {
            print!("{pretty}");
            Vec::new()
        }
    }
}

/// Resolves the targets: explicit flags, else the manifest's list, else both.
fn requested_targets(
    cli_targets: &[TargetProfile],
    bundle: &Bundle,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<Vec<TargetProfile>, ()> {
    if !cli_targets.is_empty() {
        return Ok(cli_targets.to_vec());
    }
    if bundle.manifest.targets.is_empty() {
        return Ok(vec![TargetProfile::Default, TargetProfile::Stripped]);
    }
    let mut targets = Vec::with_capacity(bundle.manifest.targets.len());
    for name in &bundle.manifest.targets {
        match name.as_str() {
            "default" => targets.push(TargetProfile::Default),
            "stripped" => targets.push(TargetProfile::Stripped),
            other => {
                diagnostics.push(Diagnostic::file(
                    crate::diagnostics::DOCUMENT_CONTRACT,
                    Severity::Error,
                    &bundle.root.join(bundle::MANIFEST_NAME).display().to_string(),
                    format!("unknown target `{other}` in the manifest; expected `default` or `stripped`"),
                ));
                return Err(());
            }
        }
    }
    Ok(targets)
}

fn function_name(bundle: &Bundle) -> &str {
    bundle.manifest.function_name.as_deref().unwrap_or("build_ui")
}

/// Writes every generated target below `root`.
///
/// Mirrors `designer::artifact::write_one`: compare before writing so an unchanged run does not
/// touch an mtime, and refuse a source without the generated marker so the drift gate can always
/// recognise what this tool wrote.
///
/// The progress line for each artifact is routed through [`progress`], so `--json` keeps stdout a
/// pure NDJSON stream (DM6-01).
fn write_all(generated: &[Generated], root: &Path, json: bool) -> Result<(), String> {
    for artifact in generated {
        if !artifact.source.contains(GENERATED_MARKER) {
            return Err(format!(
                "refusing to write {} without the generated marker",
                ArtifactPaths::path_for(artifact.target)
            ));
        }
        let absolute = root.join(ArtifactPaths::path_for(artifact.target));
        if let Ok(existing) = std::fs::read_to_string(&absolute) {
            if existing == artifact.source {
                progress(json, format!("unchanged {}", absolute.display()));
                continue;
            }
            if !is_generated(&existing) {
                return Err(format!(
                    "{} exists but is not a generated file; refusing to overwrite it",
                    absolute.display()
                ));
            }
        }
        if let Some(parent) = absolute.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("could not create {}: {error}", parent.display()))?;
        }
        let existed = absolute.exists();
        std::fs::write(&absolute, &artifact.source)
            .map_err(|error| format!("could not write {}: {error}", absolute.display()))?;
        progress(
            json,
            format!("{} {}", if existed { "updated" } else { "created" }, absolute.display()),
        );
    }
    Ok(())
}

/// Prints one progress line: stdout normally, stderr under `--json`.
///
/// The README promises that `--json` writes one diagnostic object per line (NDJSON) to stdout, so
/// a progress line there breaks every consumer that parses stdout line by line — and it did so on
/// the *success* path, which is the automation path (DM6-01). The information still has to reach
/// a person, so it moves to stderr instead of being dropped.
fn progress(json: bool, line: String) {
    if json {
        eprintln!("{line}");
    } else {
        println!("{line}");
    }
}

fn io_diagnostic(path: &Path, error: impl std::fmt::Display) -> Diagnostic {
    Diagnostic::file(IO_FAILED, Severity::Error, &path.display().to_string(), error.to_string())
}

/// Prints the diagnostics in the selected form.
fn report(diagnostics: &[Diagnostic], json: bool) {
    if json {
        for diagnostic in diagnostics {
            println!("{}", diagnostic.to_json());
        }
        return;
    }
    for diagnostic in diagnostics {
        eprintln!("{}", diagnostic.to_text());
    }
}
