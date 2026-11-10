// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Bundle and manifest loading (BLUE31 §2.2).
//!
//! A *bundle* is the recommended input shape: a directory holding the document, optional
//! stylesheets and an optional `rwui.json` manifest. A bare `.json` document is also accepted;
//! then the bundle root is the file's parent, so `-o` stays optional.

use std::path::{Path, PathBuf};

/// The manifest's name inside a bundle directory.
pub const MANIFEST_NAME: &str = "rwui.json";
/// The document's conventional name inside a bundle directory.
pub const DOCUMENT_NAME: &str = "ui.json";
/// The stylesheet's conventional name inside a bundle directory.
pub const THEME_NAME: &str = "theme.css";

/// The manifest's fields, all optional except the document it names (which defaults).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Manifest {
    /// Manifest format version; only `1` is understood.
    pub version: Option<u64>,
    /// Document file, relative to the bundle root.
    pub document: Option<String>,
    /// Stylesheets, relative to the bundle root.
    pub stylesheets: Vec<String>,
    /// Targets to produce: `default` and/or `stripped`.
    pub targets: Vec<String>,
    /// Client width override.
    pub width: Option<u32>,
    /// Client height override.
    pub height: Option<u32>,
    /// Name of the generated function.
    pub function_name: Option<String>,
    /// Directory written below the bundle root, relative to it.
    pub output_dir: Option<String>,
    /// Whether warnings that describe gaps count as errors.
    pub strict: Option<bool>,
}

/// A resolved input: where the root is, which document to read, and which sheets apply.
#[derive(Debug, Clone)]
pub struct Bundle {
    /// The directory a manifest's relative paths are resolved against.
    pub root: PathBuf,
    /// The document to read.
    pub document: PathBuf,
    /// Stylesheets to read, in manifest order, with `theme.css` appended when present.
    pub stylesheets: Vec<PathBuf>,
    /// The manifest, defaulted when the input was a bare document.
    pub manifest: Manifest,
}

/// The command-line stylesheet overrides.
#[derive(Debug, Clone, Default)]
pub struct Overrides {
    /// `--css` values, in the order given.
    pub stylesheets: Vec<PathBuf>,
    /// `-o` output root.
    pub output_dir: Option<PathBuf>,
}

/// Loads a bundle from a directory or a document path.
///
/// # Errors
///
/// Returns the OS error text when a required path cannot be read or a manifest cannot be parsed.
/// JSON-syntax errors in the manifest are reported here rather than as diagnostics because the
/// manifest is the CLI's own input, not the project document (BLUE31 §2.8 draws that line).
pub fn load(input: &Path, overrides: &Overrides) -> Result<Bundle, String> {
    if input.is_dir() {
        load_directory(input, overrides)
    } else {
        Ok(Bundle {
            root: input.parent().map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from(".")),
            document: input.to_path_buf(),
            stylesheets: overrides.stylesheets.clone(),
            manifest: Manifest::default(),
        })
    }
}

fn load_directory(root: &Path, overrides: &Overrides) -> Result<Bundle, String> {
    let manifest_path = root.join(MANIFEST_NAME);
    let manifest = if manifest_path.is_file() {
        let text = std::fs::read_to_string(&manifest_path)
            .map_err(|error| format!("could not read {}: {error}", manifest_path.display()))?;
        parse_manifest(&text, &manifest_path)?
    } else {
        Manifest::default()
    };

    if let Some(version) = manifest.version {
        if version != 1 {
            return Err(format!(
                "{}: unsupported manifest version {version}; this rwui understands version 1",
                manifest_path.display()
            ));
        }
    }

    let document = root.join(manifest.document.as_deref().unwrap_or(DOCUMENT_NAME));
    let mut stylesheets: Vec<PathBuf> =
        manifest.stylesheets.iter().map(|sheet| root.join(sheet)).collect();
    let theme = root.join(THEME_NAME);
    if theme.is_file() && !stylesheets.iter().any(|sheet| sheet == &theme) {
        stylesheets.push(theme);
    }
    stylesheets.extend(overrides.stylesheets.iter().cloned());

    Ok(Bundle { root: root.to_path_buf(), document, stylesheets, manifest })
}

fn parse_manifest(text: &str, path: &Path) -> Result<Manifest, String> {
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|error| format!("{}: {error}", path.display()))?;
    let object = value
        .as_object()
        .ok_or_else(|| format!("{}: the manifest must be a JSON object", path.display()))?;

    let mut stylesheets = Vec::new();
    if let Some(sheets) = object.get("stylesheets") {
        let array = sheets
            .as_array()
            .ok_or_else(|| format!("{}: `stylesheets` must be an array", path.display()))?;
        for sheet in array {
            let name = sheet.as_str().ok_or_else(|| {
                format!("{}: every `stylesheets` entry must be a string", path.display())
            })?;
            stylesheets.push(String::from(name));
        }
    }
    let mut targets = Vec::new();
    if let Some(listed) = object.get("targets") {
        let array = listed
            .as_array()
            .ok_or_else(|| format!("{}: `targets` must be an array", path.display()))?;
        for target in array {
            let name = target.as_str().ok_or_else(|| {
                format!("{}: every `targets` entry must be a string", path.display())
            })?;
            targets.push(String::from(name));
        }
    }

    Ok(Manifest {
        version: object.get("version").and_then(|value| value.as_u64()),
        document: object.get("document").and_then(|value| value.as_str()).map(String::from),
        stylesheets,
        targets,
        width: object
            .get("width")
            .and_then(|value| value.as_u64())
            .and_then(|value| u32::try_from(value).ok()),
        height: object
            .get("height")
            .and_then(|value| value.as_u64())
            .and_then(|value| u32::try_from(value).ok()),
        function_name: object
            .get("function_name")
            .and_then(|value| value.as_str())
            .map(String::from),
        output_dir: object.get("output_dir").and_then(|value| value.as_str()).map(String::from),
        strict: object.get("strict").and_then(|value| value.as_bool()),
    })
}

impl Bundle {
    /// The directory generation writes into (`output_dir`, else the bundle root).
    pub fn output_root(&self, overrides: &Overrides) -> PathBuf {
        if let Some(dir) = &overrides.output_dir {
            return dir.clone();
        }
        match &self.manifest.output_dir {
            Some(dir) => self.root.join(dir),
            None => self.root.clone(),
        }
    }

    /// Whether the bundle declares any style source the CLI cannot yet apply (BLUE31 §2.4 P1).
    pub fn has_stylesheets(&self) -> bool {
        !self.stylesheets.is_empty()
    }
}
