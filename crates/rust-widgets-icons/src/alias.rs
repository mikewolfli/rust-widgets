// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Designer semantic aliases over the generated [`IconName`] table.
//!
//! These are **input-only** spellings: [`IconName::from_name_or_alias`] accepts them, while
//! [`IconName::from_name`], [`IconName::as_str`] and [`IconName::ALL`] keep describing the
//! glyph-bearing variants exactly.

use crate::IconName;

/// Designer semantic names that an existing, truthful glyph can stand in for.
///
/// # Why these are aliases and not enum variants
///
/// Every `IconName` variant indexes a row of the data and fallback tables; adding a variant
/// without a vendored outline is exactly the "invent SVG data silently" failure the icon pipeline
/// is built to prevent. The designer vocabulary contains names this crate has no distinct outline
/// for. For the entries below, one of the shipped glyphs *is* the same idea, so accepting the
/// designer spelling and drawing that glyph is honest.
pub const DESIGNER_ALIASES: [(&str, IconName); 19] = [
    // A library of assets is a folder of files.
    ("assets-library", IconName::Folder),
    // A pipeline is a connected graph; `share` is this crate's node-and-branches glyph.
    ("build-pipeline", IconName::Share),
    // A build target is something that runs.
    ("build-target", IconName::Play),
    // A source map is a file that describes a source document.
    ("code-source-map", IconName::File),
    // The conventional debugger breakpoint mark is a filled square.
    ("debug-breakpoint", IconName::Stop),
    // A canvas is an editable surface.
    ("design-canvas", IconName::Edit),
    // A flow is a connected graph.
    ("design-flow", IconName::Share),
    // Inspecting is searching.
    ("design-inspect", IconName::Search),
    // A layout is a grid.
    ("design-layout", IconName::Table),
    // A theme is configured with a gear.
    ("design-theme", IconName::Settings),
    // A preview matrix is a grid of cells.
    ("preview-matrix", IconName::Table),
    // The AI/sparkle convention is a star in this vocabulary.
    ("tools-ai", IconName::Star),
    // Version-control branches are a connected graph.
    ("vcs-branch", IconName::Share),
    // The six designer names that have their own real outlines.
    ("debug-bug", IconName::DebugBug),
    ("design-component", IconName::DesignComponent),
    ("design-event", IconName::DesignEvent),
    ("test-flask", IconName::TestFlask),
    ("tools-terminal", IconName::ToolsTerminal),
    ("tools-translate", IconName::ToolsTranslate),
];

/// Resolves a designer semantic name to the truthful glyph that stands in for it.
///
/// Matching is ASCII case-sensitive and accepts both the hyphenated designer spelling and the
/// underscore spelling used by `IconName` tokens (`vcs-branch` / `vcs_branch`). A name that is not
/// in [`DESIGNER_ALIASES`] returns `None`; no fuzzy or prefix matching is performed, since guessing
/// at an icon is worse than drawing the documented placeholder.
pub fn designer_alias(name: &str) -> Option<IconName> {
    DESIGNER_ALIASES.iter().find_map(|(alias, target)| {
        if alias_spelling_matches(name, alias) {
            Some(*target)
        } else {
            None
        }
    })
}

/// Whether `name` is a designer alias this crate understands.
pub fn is_designer_alias(name: &str) -> bool {
    designer_alias(name).is_some()
}

/// Compares an input spelling to a table alias, treating `-` and `_` as the same separator.
///
/// Byte-wise rather than `to_ascii_lowercase`/`replace` so a draw-time lookup allocates nothing.
fn alias_spelling_matches(input: &str, alias: &str) -> bool {
    input.len() == alias.len()
        && input.bytes().zip(alias.bytes()).all(|(input, alias)| {
            input == alias || (input == b'_' && alias == b'-') || (input == b'-' && alias == b'_')
        })
}

impl IconName {
    /// Resolves a canonical token **or** a designer alias to a glyph-bearing variant.
    ///
    /// [`IconName::from_name`] is generated from the token list and consequently knows only the
    /// enum's own tokens. This method adds the alias table on top.
    pub fn from_name_or_alias(name: &str) -> Option<IconName> {
        IconName::from_name(name).or_else(|| designer_alias(name))
    }

    /// The designer semantic names accepted by [`IconName::from_name_or_alias`], with their
    /// glyphs.
    pub fn designer_aliases() -> &'static [(&'static str, IconName)] {
        &DESIGNER_ALIASES
    }

    /// The canonical token of the glyph a designer alias reuses, or `None` when the name is not
    /// an alias.
    pub fn canonical_token_for_alias(name: &str) -> Option<&'static str> {
        designer_alias(name).map(|icon| icon.as_str())
    }
}
