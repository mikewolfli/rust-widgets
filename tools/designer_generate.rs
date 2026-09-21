// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The designer's generation entry point — the program side of D7-b-3.
//!
//! # What this is
//!
//! BLUE19's D7-b-3 decision is that generated sources are **committed**, and that the designer calls
//! the generator to produce them. This binary is that call: a designer shell (or a build script, or a
//! person) runs
//!
//! ```text
//! cargo run --no-default-features --features desktop,designer --example designer_generate -- \
//!     --project ui/project.json --root . --width 1280 --height 800
//! ```
//!
//! and the two committed files are written. `tools/check_generated_sources.sh` regenerates them and
//! compares, which is what makes committing safe.
//!
//! # Why a binary and not only the library API
//!
//! A designer **embeds** `rust_widgets` and calls [`regenerate_into`] in-process, so it gets the
//! outcome list and can show "no change" in its status area. But the same generation must be
//! reachable from a shell: that is what a build script does, what a reviewer runs to check a
//! colleague's committed artifact, and what the gate runs. Shipping only the in-process API would
//! force every one of those to re-implement the argument handling.
//!
//! # Why the output is machine-readable
//!
//! One line per file, `created|updated|unchanged <path>`, then a summary line. A designer's status
//! area and a shell script both read that without parsing prose — and a `--check` run's exit status
//! is the gate's criterion, so the text is for the human reading the log.
//!
//! [`regenerate_into`]: rust_widgets::designer::artifact::regenerate_into

use rust_widgets::designer::artifact::{
    plan_artifacts, regenerate_into, ArtifactOutcome, GENERATED_MARKER,
};
use std::process::ExitCode;

/// The exit status for "a committed artifact does not match its project document".
///
/// Distinct from a hard error (2): a stale file is a **finding**, not a broken tool, and a caller may
/// want to treat them differently — CI reports both, a designer's status area only the first.
const EXIT_DRIFT: u8 = 1;

/// The exit status for "the tool could not do its job" (bad arguments, unreadable project,
/// unparsable JSON).
const EXIT_USAGE: u8 = 2;

fn main() -> ExitCode {
    let options = match Options::parse(std::env::args().skip(1)) {
        Ok(options) => options,
        Err(error) => {
            eprintln!("designer_generate: {error}");
            eprintln!();
            eprintln!("{USAGE}");
            return ExitCode::from(EXIT_USAGE);
        }
    };

    match run(&options) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(EXIT_DRIFT),
        Err(error) => {
            eprintln!("designer_generate: {error}");
            ExitCode::from(EXIT_USAGE)
        }
    }
}

const USAGE: &str = "\
usage: designer_generate --project <file> [--root <dir>] [--width <px>] [--height <px>] [--check]

  --project <file>   the designer's JSON project (required)
  --root <dir>       where `src/generated/` lives (default: the current directory)
  --width <px>       the client width the layout is solved against (default: 1280)
  --height <px>      the client height the layout is solved against (default: 800)
  --check            do not write; report whether the committed files are up to date

exit status:
  0  every committed file matches the project (or was written)
  1  --check found a committed file that does not match
  2  the arguments or the project document could not be used";

/// The parsed command line.
struct Options {
    project: String,
    root: std::path::PathBuf,
    width: u32,
    height: u32,
    check: bool,
}

impl Options {
    fn parse(args: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut project: Option<String> = None;
        let mut root = std::path::PathBuf::from(".");
        let mut width = 1280u32;
        let mut height = 800u32;
        let mut check = false;

        let mut args = args.peekable();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--project" => {
                    project = Some(args.next().ok_or("--project needs a file path")?);
                }
                "--root" => {
                    root = std::path::PathBuf::from(args.next().ok_or("--root needs a directory")?);
                }
                "--width" => {
                    width = parse_dimension("--width", args.next())?;
                }
                "--height" => {
                    height = parse_dimension("--height", args.next())?;
                }
                "--check" => check = true,
                "--help" | "-h" => return Err(String::from("(help requested)")),
                other => return Err(format!("unknown argument `{other}`")),
            }
        }

        Ok(Self { project: project.ok_or("--project is required")?, root, width, height, check })
    }
}

/// Parses a dimension, naming the flag in the error so a typo is locatable.
fn parse_dimension(flag: &str, value: Option<String>) -> Result<u32, String> {
    let value = value.ok_or_else(|| format!("{flag} needs a pixel count"))?;
    value
        .parse::<u32>()
        .map_err(|_| format!("{flag} must be a non-negative integer, got `{value}`"))
}

/// Runs the generation, returning `true` when every file is up to date.
fn run(options: &Options) -> Result<bool, String> {
    let json = std::fs::read_to_string(&options.project)
        .map_err(|error| format!("could not read `{}`: {error}", options.project))?;

    if options.check {
        return check_committed(options, &json);
    }

    let outcomes = regenerate_into(&options.root, &json, options.width, options.height)?;
    report(&outcomes);
    Ok(true)
}

/// Compares the committed files against what the project would generate, writing nothing.
///
/// # Why this does not write and then `git diff`
///
/// A caller could regenerate and let version control report the change, but that mutates the working
/// tree — which is wrong for a check, and wrong for a designer that only wants to know whether the
/// file it is about to overwrite is stale. Comparing in memory keeps `--check` free of side effects,
/// which is what lets the gate run it on a read-only checkout.
fn check_committed(options: &Options, json: &str) -> Result<bool, String> {
    let artifacts = plan_artifacts(json, options.width, options.height)?;
    let mut up_to_date = true;

    for artifact in &artifacts {
        let absolute = options.root.join(&artifact.path);
        match std::fs::read_to_string(&absolute) {
            Ok(committed) if committed == artifact.source => {
                println!("unchanged {}", absolute.display());
            }
            Ok(committed) if !committed.contains(GENERATED_MARKER) => {
                // A file without the marker is not one this tool wrote. Saying so is more useful than
                // reporting a diff against a hand-written module that happens to share the path.
                eprintln!(
                    "designer_generate: {} exists but is not a generated file (its first line is \
                     not the generated marker); refusing to compare it",
                    absolute.display()
                );
                up_to_date = false;
            }
            Ok(_) => {
                println!("stale {}", absolute.display());
                up_to_date = false;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                println!("missing {}", absolute.display());
                up_to_date = false;
            }
            Err(error) => {
                return Err(format!("could not read {}: {error}", absolute.display()));
            }
        }
    }

    if !up_to_date {
        eprintln!();
        eprintln!(
            "designer_generate: committed sources do not match `{}`. Regenerate with:\n\
             \x20 cargo run --no-default-features --features desktop,designer --example designer_generate \
             -- --project {} --root {}",
            options.project,
            options.project,
            options.root.display()
        );
    }
    Ok(up_to_date)
}

/// Prints the outcome of a write run, one line per file.
fn report(outcomes: &[ArtifactOutcome]) {
    for outcome in outcomes {
        let verb = match outcome {
            ArtifactOutcome::Created(_) => "created",
            ArtifactOutcome::Updated(_) => "updated",
            ArtifactOutcome::Unchanged(_) => "unchanged",
        };
        println!("{verb} {}", outcome.path());
    }
    let changed = outcomes.iter().filter(|outcome| outcome.wrote()).count();
    println!(
        "{} file(s): {} written, {} already up to date",
        outcomes.len(),
        changed,
        outcomes.len() - changed
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| String::from(*item)).collect()
    }

    #[test]
    fn the_project_flag_is_required() {
        let error = Options::parse(args(&[]).into_iter()).unwrap_err();
        assert!(error.contains("--project is required"), "got: {error}");
    }

    #[test]
    fn the_defaults_are_a_real_window_size_and_the_current_directory() {
        let options = Options::parse(args(&["--project", "p.json"]).into_iter()).expect("parses");
        assert_eq!(options.width, 1280);
        assert_eq!(options.height, 800);
        assert_eq!(options.root, std::path::PathBuf::from("."));
        assert!(!options.check, "writing is the default; `--check` is the opt-in");
    }

    #[test]
    fn every_flag_is_accepted() {
        let options = Options::parse(
            args(&[
                "--project",
                "p.json",
                "--root",
                "/tmp/x",
                "--width",
                "320",
                "--height",
                "240",
                "--check",
            ])
            .into_iter(),
        )
        .expect("parses");
        assert_eq!(options.project, "p.json");
        assert_eq!(options.root, std::path::PathBuf::from("/tmp/x"));
        assert_eq!(options.width, 320);
        assert_eq!(options.height, 240);
        assert!(options.check);
    }

    #[test]
    fn a_non_numeric_dimension_names_its_flag() {
        let error =
            Options::parse(args(&["--project", "p", "--width", "wide"]).into_iter()).unwrap_err();
        assert!(error.contains("--width"), "got: {error}");
        assert!(error.contains("wide"), "the offending value must appear: {error}");
    }

    #[test]
    fn a_flag_missing_its_value_is_refused() {
        let error = Options::parse(args(&["--project"]).into_iter()).unwrap_err();
        assert!(error.contains("needs a file path"), "got: {error}");
    }

    #[test]
    fn an_unknown_flag_is_refused_rather_than_ignored() {
        // Silently ignoring a typo (`--chekc`) would run a full write when the caller asked for a
        // read-only check — the opposite of what they wanted, with no indication.
        let error = Options::parse(args(&["--project", "p", "--chekc"]).into_iter()).unwrap_err();
        assert!(error.contains("unknown argument"), "got: {error}");
    }

    #[test]
    fn a_negative_dimension_is_refused() {
        let error =
            Options::parse(args(&["--project", "p", "--width", "-1"]).into_iter()).unwrap_err();
        assert!(error.contains("--width"), "got: {error}");
    }
}
