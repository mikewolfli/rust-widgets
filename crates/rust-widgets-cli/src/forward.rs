// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The forward direction: `json (+ css) → rust` (BLUE31 §2.4).
//!
//! Stage P0 covers the document path end to end: parse, size resolution, `designer::generate`
//! per target, diagnostics, and the drift comparison `--check` performs. Static CSS resolution is
//! stage P1 and is reported rather than silently ignored when a bundle supplies stylesheets.

use crate::diagnostics::{
    Diagnostic, Severity, ARTIFACT_DRIFT, CAPACITY_OVERFLOW, DOCUMENT_CONTRACT,
    GENERATOR_UNSUPPORTED, JSON_PARSE_FAILED, STYLE_GAP,
};
use rust_widgets::designer::{
    generate, is_generated, ArtifactPaths, GenerationGap, GenerationRequest, TargetProfile,
};
use rust_widgets::json::JsonProject;
use std::path::Path;

/// The solve size used when neither the manifest nor the document states one.
///
/// These are the same fallbacks `tools/designer_generate.rs` documents and uses, so the two
/// generators cannot disagree about a document that declares no window size.
pub const FALLBACK_WIDTH: u32 = 1280;
/// See [`FALLBACK_WIDTH`].
pub const FALLBACK_HEIGHT: u32 = 800;

/// One generated target, held in memory so `--check` can compare without writing.
pub struct Generated {
    /// Which template produced it.
    pub target: TargetProfile,
    /// The source text, including the generated marker.
    pub source: String,
}

/// The result of a forward run.
pub struct ForwardOutcome {
    /// Findings, in the order they were discovered.
    pub diagnostics: Vec<Diagnostic>,
    /// One entry per target.
    pub generated: Vec<Generated>,
}

/// Parses the document, reporting JSON syntax and contract failures with their own codes.
pub fn parse_document(json: &str, file: &Path) -> Result<JsonProject, Vec<Diagnostic>> {
    let file = file.display().to_string();
    // `serde_json`'s error carries the location, which is the whole point of RWCLI-001 being a
    // separate code from RWCLI-002: a syntax error is line-addressed, a contract error is not.
    if let Err(error) = serde_json::from_str::<serde_json::Value>(json) {
        return Err(vec![Diagnostic::at(
            JSON_PARSE_FAILED,
            Severity::Error,
            &file,
            error.line() as u32,
            error.column() as u32,
            error.to_string(),
        )]);
    }
    JsonProject::parse(json)
        .map_err(|error| vec![Diagnostic::file(DOCUMENT_CONTRACT, Severity::Error, &file, error)])
}

/// Resolves the client size: manifest override, then the document, then the shared fallback.
pub fn solve_size(project: &JsonProject, width: Option<u32>, height: Option<u32>) -> (u32, u32) {
    let declared = project.window_size();
    (
        width.or(declared.map(|(width, _)| width)).unwrap_or(FALLBACK_WIDTH),
        height.or(declared.map(|(_, height)| height)).unwrap_or(FALLBACK_HEIGHT),
    )
}

/// Generates every requested target and collects the report's gaps as diagnostics.
pub fn generate_targets(
    json: &str,
    project: &JsonProject,
    file: &Path,
    size: (u32, u32),
    function_name: &str,
    targets: &[TargetProfile],
    strict: bool,
) -> ForwardOutcome {
    let file = file.display().to_string();
    let mut diagnostics = Vec::new();
    let mut generated = Vec::with_capacity(targets.len());

    for target in targets {
        let request = GenerationRequest {
            json: String::from(json),
            target: *target,
            width: size.0,
            height: size.1,
            function_name: String::from(function_name),
        };
        match generate(&request) {
            Ok(source) => {
                collect_report(
                    &source.report.unsupported,
                    GENERATOR_UNSUPPORTED,
                    "the generator refused this node",
                    &project.root_widget,
                    &file,
                    strict,
                    &mut diagnostics,
                );
                collect_report(
                    &source.report.capacity_overflow,
                    CAPACITY_OVERFLOW,
                    "the container exceeds the target's child capacity",
                    &project.root_widget,
                    &file,
                    strict,
                    &mut diagnostics,
                );
                // Caveats are "could not settle availability under this profile": a note, never a
                // failure. The plan's code table has no dedicated code, so they travel under the
                // generator-unsupported code at note severity (recorded in the round log).
                for caveat in &source.report.cross_profile_caveats {
                    diagnostics.push(
                        Diagnostic::file(
                            GENERATOR_UNSUPPORTED,
                            Severity::Note,
                            &file,
                            format!("cross-profile caveat: {}", caveat.reason),
                        )
                        .with_node(gap_path(project, caveat)),
                    );
                }
                generated.push(Generated { target: *target, source: source.source });
            }
            Err(error) => {
                diagnostics.push(Diagnostic::file(DOCUMENT_CONTRACT, Severity::Error, &file, error))
            }
        }
    }

    ForwardOutcome { diagnostics, generated }
}

fn collect_report(
    gaps: &[GenerationGap],
    code: &'static str,
    fallback: &str,
    root_widget: &str,
    file: &str,
    strict: bool,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for gap in gaps {
        let severity = if strict { Severity::Error } else { Severity::Warning };
        let message =
            if gap.reason.is_empty() { String::from(fallback) } else { gap.reason.clone() };
        diagnostics.push(
            Diagnostic::file(code, severity, file, message)
                .with_node(format_gap_path(root_widget, &gap.path)),
        );
    }
}

fn gap_path(project: &JsonProject, gap: &GenerationGap) -> String {
    format_gap_path(&project.root_widget, &gap.path)
}

/// Formats a tree path the way BLUE31 §2.8 spells it: `<root>/[0]/[2]`.
pub fn format_gap_path(root_widget: &str, path: &[usize]) -> String {
    let mut out = String::from(root_widget);
    for index in path {
        out.push_str(&format!("/[{index}]"));
    }
    out
}

/// Reports stylesheets that stage P0 cannot apply yet.
///
/// This is a gap, not a success: the document's explicit keys still generate, but every rule in
/// the sheets is unapplied. `--strict` turns it into an error, which is what a caller who requires
/// CSS fidelity needs (BLUE31 §2.5's "unknown things are reported, not silently ignored").
pub fn stylesheet_gap(sheets: &[std::path::PathBuf], strict: bool) -> Vec<Diagnostic> {
    sheets
        .iter()
        .map(|sheet| {
            Diagnostic::file(
                STYLE_GAP,
                if strict { Severity::Error } else { Severity::Warning },
                &sheet.display().to_string(),
                String::from(
                    "static CSS cascade is not applied by this stage; generation used the \
                     document's explicit keys only",
                ),
            )
            .with_hint(String::from(
                "pass the document's explicit keys, or wait for the CSS stage (BLUE31 §2.4 P1)",
            ))
        })
        .collect()
}

/// Compares generated sources against what is committed below `root`.
///
/// Returns one `RWCLI-011` diagnostic per file that is missing, hand-written, or stale. An
/// unreadable file is an I/O failure and is returned as `Err`.
pub fn check_committed(
    generated: &[Generated],
    root: &Path,
) -> Result<Vec<Diagnostic>, std::io::Error> {
    let mut diagnostics = Vec::new();
    for artifact in generated {
        let relative = ArtifactPaths::path_for(artifact.target);
        let absolute = root.join(&relative);
        let file = absolute.display().to_string();
        match std::fs::read_to_string(&absolute) {
            Ok(committed) if committed == artifact.source => {}
            Ok(committed) if !is_generated(&committed) => diagnostics.push(
                Diagnostic::file(
                    ARTIFACT_DRIFT,
                    Severity::Error,
                    &file,
                    String::from("the file exists but is not one this generator wrote"),
                )
                .with_hint(String::from(
                    "remove the hand-written file or write the generated one in its place",
                )),
            ),
            Ok(_) => diagnostics.push(Diagnostic::file(
                ARTIFACT_DRIFT,
                Severity::Error,
                &file,
                String::from("the committed file does not match the document"),
            )),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                diagnostics.push(Diagnostic::file(
                    ARTIFACT_DRIFT,
                    Severity::Error,
                    &file,
                    String::from("the generated file is missing"),
                ));
            }
            Err(error) => return Err(error),
        }
    }
    Ok(diagnostics)
}
