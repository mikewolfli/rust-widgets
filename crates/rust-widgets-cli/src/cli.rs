// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Hand-written argument parsing (BLUE31 §2.11: no `clap`).
//!
//! Only the commands this stage implements are accepted. A command the plan lists for a later
//! stage is *unknown* here rather than a silent no-op, which is the difference between "the tool
//! cannot do this yet" and "the tool did it and produced nothing".

use crate::bundle::Overrides;
use rust_widgets::designer::TargetProfile;
use std::path::PathBuf;

/// What the caller asked for.
#[derive(Debug, Clone)]
pub enum Command {
    /// Validate a bundle; write nothing.
    Check {
        /// The bundle directory or document file.
        input: PathBuf,
    },
    /// Generate Rust from the document.
    Json2Rust {
        /// The bundle directory or document file.
        input: PathBuf,
        /// `--css` / `-o` overrides.
        overrides: Overrides,
        /// Explicit `--target` values; empty means "the manifest's, else both".
        targets: Vec<TargetProfile>,
        /// Compare against the committed files instead of writing.
        check: bool,
    },
    /// Rewrite the document with stable key order and indentation.
    Fmt {
        /// The document to normalise.
        input: PathBuf,
        /// Where to write; `None` means stdout.
        output: Option<PathBuf>,
    },
    /// Print the usage text.
    Help,
}

/// The parsed command line.
#[derive(Debug, Clone)]
pub struct Options {
    /// The command to run.
    pub command: Command,
    /// Emit NDJSON diagnostics instead of text.
    pub json: bool,
    /// Treat gap warnings as errors.
    pub strict: bool,
}

/// The usage text.
pub const USAGE: &str = "\
rwui — the rust_widgets designer CLI

Usage:
  rwui check <bundle-or-json> [--json] [--strict]
  rwui json2rust <bundle-or-json> [-o <dir>] [--css <file>]... [--target default|stripped]
                                  [--check] [--json] [--strict]
  rwui fmt <document.json> [-o <file>]

A bundle is a directory containing `ui.json` (or the `document` named by `rwui.json`),
optionally `theme.css`, `rwui.json` and `assets/`.

Exit status:
  0  no errors
  1  diagnostics reported an error (or `--strict` promoted a gap to one)
  2  the tool could not run (bad arguments, unreadable input)";

/// The flags shared by `check` and `json2rust`.
#[derive(Default)]
struct Shared {
    json: bool,
    strict: bool,
    overrides: Overrides,
    targets: Vec<TargetProfile>,
    check: bool,
}

/// Parses `args` (without the program name).
pub fn parse(args: impl Iterator<Item = String>) -> Result<Options, String> {
    let mut args = args.into_iter().peekable();
    let mut shared = Shared::default();

    // Global flags are accepted before the command as well as after it.
    let command_name = loop {
        match args.next() {
            Some(flag) if flag == "--json" => shared.json = true,
            Some(flag) if flag == "--strict" => shared.strict = true,
            Some(flag) if flag == "--help" || flag == "-h" => break String::from("help"),
            Some(name) if name.starts_with('-') => return Err(format!("unknown flag `{name}`")),
            Some(name) => break name,
            None => return Err(String::from("no command given")),
        }
    };

    let command = match command_name.as_str() {
        "help" => Command::Help,
        "check" => {
            let input = first_positional(&mut args)?;
            parse_flags(args, &mut shared, false)?;
            Command::Check { input }
        }
        "json2rust" => {
            let input = first_positional(&mut args)?;
            parse_flags(args, &mut shared, true)?;
            Command::Json2Rust {
                input,
                overrides: shared.overrides,
                targets: shared.targets,
                check: shared.check,
            }
        }
        "fmt" => {
            let input = first_positional(&mut args)?;
            let mut output = None;
            while let Some(flag) = args.next() {
                match flag.as_str() {
                    "-o" | "--output" => {
                        output = Some(PathBuf::from(args.next().ok_or("-o needs a file path")?));
                    }
                    other => return Err(format!("unknown argument `{other}`")),
                }
            }
            Command::Fmt { input, output }
        }
        other => return Err(format!("unknown command `{other}`")),
    };

    Ok(Options { command, json: shared.json, strict: shared.strict })
}

fn first_positional(args: &mut impl Iterator<Item = String>) -> Result<PathBuf, String> {
    let value = args.next().ok_or("the command needs an input path")?;
    if value.starts_with('-') {
        return Err(format!("expected an input path, got the flag `{value}`"));
    }
    Ok(PathBuf::from(value))
}

fn parse_flags(
    mut args: impl Iterator<Item = String>,
    shared: &mut Shared,
    allow_targets: bool,
) -> Result<(), String> {
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--json" => shared.json = true,
            "--strict" => shared.strict = true,
            "--check" => shared.check = true,
            "--css" => shared
                .overrides
                .stylesheets
                .push(PathBuf::from(args.next().ok_or("--css needs a file path")?)),
            "-o" | "--output" => {
                shared.overrides.output_dir =
                    Some(PathBuf::from(args.next().ok_or("-o needs a directory")?));
            }
            "--target" if allow_targets => {
                let name = args.next().ok_or("--target needs a name")?;
                shared.targets.push(match name.as_str() {
                    "default" => TargetProfile::Default,
                    "stripped" => TargetProfile::Stripped,
                    other => {
                        return Err(format!(
                            "unknown target `{other}`; expected `default` or `stripped`"
                        ))
                    }
                });
            }
            other => return Err(format!("unknown argument `{other}`")),
        }
    }
    Ok(())
}
