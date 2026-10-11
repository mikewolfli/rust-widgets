// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Logical resource identity: one name for an asset that resolves the same at runtime and in a
//! static package (BLUE26 B26.12).
//!
//! # What problem this solves
//!
//! Every asset entry point in this crate is keyed by a **filesystem path**
//! (`image::cache::cached_from_path`, the image/audio widgets' `*_source` properties). A path is a
//! *location*, and a location is the one thing that differs between the two places an asset is used:
//!
//! * at run time it is a file beside the executable, under the source tree, or wherever the host put
//!   it;
//! * in a static package it is often **embedded** in the binary, with no file at all.
//!
//! A designer that wires an `image_source` to a path therefore produces a project that breaks the
//! moment it is packaged. What survives both is a **logical identity** — `"icons/save.png"`,
//! `"audio/click.wav"` — that each deployment resolves its own way. This module is that identity and
//! the seam that resolves it; the *policy* (where the bytes come from) stays with the host, which is
//! the only layer that knows how the application was packaged.
//!
//! # What stays in the host
//!
//! File watching, import/transcoding, licensing, packaging and the source index are the host's (see
//! the plan's §3). This module provides the **identity and the lookup contract**, nothing more: it
//! says "a resource is addressed by this name" and "a resolver turns a name into bytes", so two
//! deployments can agree on the name while disagreeing on the storage.
//!
//! The bytes a resolver returns compose with the existing pipeline unchanged:
//! [`Image::from_bytes`](crate::image::Image::from_bytes) and the content-keyed decode layer already
//! take bytes rather than a path, so an embedded asset and a file that happens to hold the same
//! bytes share one decoded entry. What was missing was only the *name* the two deployments agree on
//! — every existing entry point ([`image::cache::cached_from_path`](crate::image::cache::cached_from_path),
//! the image widgets' `*_source` properties) is keyed by a filesystem path, which is a location and
//! therefore the one thing that differs between a run-time load and a static package.
//!
//! # Why the id is not a path
//!
//! [`ResourceId`] is a `String`, but it is deliberately **normalised as a logical name**, not as a
//! filesystem path: `\` and `//` are collapses to `/`, a leading `./` or `/` is stripped, and `..`
//! segments are rejected outright (a logical name never escapes its root). A path that happened to
//! work on one OS therefore cannot be mistaken for an id that means the same thing on another —
//! which is the cross-platform bug this type exists to make unrepresentable.

use crate::compat::{format, String, Vec};

/// A platform-neutral logical asset name.
///
/// The spelling is normalised on construction: separators are `/`, there is no leading `./` or `/`,
/// no trailing `/`, and no `..` segment. Two spellings that mean the same name therefore compare
/// equal, which is what lets a duplicated resource be detected by equality rather than by a
/// string-specific rule.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ResourceId(String);

/// Why a string is not a valid [`ResourceId`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResourceIdError {
    /// The name is empty, or became empty after normalisation.
    Empty,
    /// The name contains a `..` segment, which would escape the resource root.
    EscapesRoot,
}

impl core::fmt::Display for ResourceIdError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Empty => f.write_str("a resource id cannot be empty"),
            Self::EscapesRoot => f.write_str("a resource id cannot contain a `..` segment"),
        }
    }
}

impl ResourceId {
    /// Parses and normalises a logical resource name.
    ///
    /// # Normalisation
    ///
    /// * `\` becomes `/` (a Windows-style separator cannot make a *different* resource);
    /// * repeated `/` collapse to one;
    /// * a leading `./` or `/` and a trailing `/` are dropped;
    /// * a `..` segment is rejected rather than resolved, because a logical name has a root and a
    ///   name that escapes it is not a name for anything inside the package.
    ///
    /// The result is the canonical identity: `"./icons\\save.png"` and `"icons/save.png"` are the
    /// same [`ResourceId`], so a project that spelled one way and a package that spelled the other
    /// still resolve to the same asset.
    pub fn parse(raw: &str) -> Result<Self, ResourceIdError> {
        let mut normalized = String::new();
        let mut first = true;
        for segment in raw.split(['/', '\\']) {
            if segment.is_empty() || segment == "." {
                // An empty segment is a repeated or leading/trailing separator; `.` is the current
                // directory in a logical name, which is the same as saying nothing.
                continue;
            }
            if segment == ".." {
                return Err(ResourceIdError::EscapesRoot);
            }
            if !first {
                normalized.push('/');
            }
            normalized.push_str(segment);
            first = false;
        }
        if normalized.is_empty() {
            return Err(ResourceIdError::Empty);
        }
        Ok(Self(normalized))
    }

    /// The canonical spelling.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The final segment, for a display label (`"save.png"` for `"icons/save.png"`).
    pub fn file_name(&self) -> &str {
        self.0.rsplit('/').next().unwrap_or(&self.0)
    }

    /// The directory part, or an empty string for a top-level name.
    pub fn parent(&self) -> &str {
        match self.0.rfind('/') {
            Some(index) => &self.0[..index],
            None => "",
        }
    }
}

impl core::fmt::Display for ResourceId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Why a resource could not be resolved to bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolveError {
    /// No source in the resolver knows this id.
    NotFound {
        /// The id that was looked up.
        id: ResourceId,
    },
    /// More than one source claims the id, so the identity is ambiguous.
    ///
    /// # Why ambiguity is an error rather than "first wins"
    ///
    /// Two sources claiming one id is exactly the "重名" case the plan names: the same logical name
    /// resolving to two different assets. Picking one silently would make the asset depend on source
    /// order, so a package that reordered its sources would show a different picture with no
    /// complaint. Reporting it is the only answer a caller can act on.
    Ambiguous {
        /// The id claimed by more than one source.
        id: ResourceId,
        /// How many sources claimed it.
        claimants: usize,
    },
    /// A source knew the id but could not produce the bytes.
    Failed {
        /// The id that failed.
        id: ResourceId,
        /// The source's own message.
        reason: String,
    },
}

impl core::fmt::Display for ResolveError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NotFound { id } => write!(f, "no source resolves the resource `{id}`"),
            Self::Ambiguous { id, claimants } => write!(
                f,
                "{claimants} sources claim the resource `{id}`, so its identity is ambiguous"
            ),
            Self::Failed { id, reason } => write!(f, "the source for `{id}` failed: {reason}"),
        }
    }
}

/// One place bytes for a logical id can come from.
///
/// A host implements this once per storage it uses — a directory beside the executable, an embedded
/// table, a downloaded cache — and the widgets resolve through the host's resolvers rather than a
/// path. `None` from [`get`](Self::get) means "not mine", which lets several sources coexist without
/// a registry that must be told what each one holds.
pub trait ResourceSource {
    /// This source's name, for a report.
    fn name(&self) -> &str;

    /// Whether this source knows `id`.
    ///
    /// Separate from [`get`](Self::get) so provenance can be decided without producing bytes — a
    /// designer showing "where does this asset come from?" must not decode it to find out.
    fn contains(&self, id: &ResourceId) -> bool;

    /// The bytes for `id`, or `None` when this source does not hold it.
    fn get(&self, id: &ResourceId) -> Option<Vec<u8>>;
}

/// An in-memory source, for a resource embedded in a static package.
///
/// # Why this is in the library
///
/// The embedding itself is the host's build step (the plan's §3 keeps packaging out of the widget
/// layer), but the *shape* of an embedded table is fixed by the resolution contract: a table of
/// entries that answers `contains`/`get` without touching a filesystem. Shipping one implementation
/// means every host that embeds does not have to invent — and mis-invent — it, and it is the source
/// a generated program's `include_bytes!` table wraps.
#[derive(Debug, Clone, Default)]
pub struct EmbeddedSource {
    name: String,
    // Keyed by the id's canonical spelling so a lookup is exact; the map is the host's table.
    entries: crate::compat::HashMap<String, Vec<u8>>,
}

impl EmbeddedSource {
    /// An empty in-memory source with `name`.
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), entries: crate::compat::HashMap::new() }
    }

    /// Adds or replaces the bytes for `id`.
    pub fn insert(&mut self, id: ResourceId, bytes: Vec<u8>) {
        self.entries.insert(id.as_str().into(), bytes);
    }

    /// Everything this source holds, sorted by id, for a report or a completeness check.
    pub fn ids(&self) -> Vec<ResourceId> {
        let mut ids: Vec<ResourceId> =
            self.entries.keys().filter_map(|key| ResourceId::parse(key).ok()).collect();
        ids.sort();
        ids
    }
}

impl ResourceSource for EmbeddedSource {
    fn name(&self) -> &str {
        &self.name
    }

    fn contains(&self, id: &ResourceId) -> bool {
        self.entries.contains_key(id.as_str())
    }

    fn get(&self, id: &ResourceId) -> Option<Vec<u8>> {
        self.entries.get(id.as_str()).cloned()
    }
}

/// Resolves a logical id through an ordered list of sources.
///
/// # Order is not a priority
///
/// The sources are a set, not a stack: the resolver asks **all** of them and reports
/// [`ResolveError::Ambiguous`] when more than one holds the id. It does **not** let the first
/// claimant win, because "first wins" makes the answer depend on the order the host happened to
/// register its sources — a package that reordered them would show a different asset silently. An
/// ordered lookup is available through [`Self::resolve_preferring`] for a host that genuinely means
/// one source to shadow another.
#[derive(Default)]
pub struct ResourceResolver {
    sources: Vec<crate::compat::Box<dyn ResourceSource>>,
}

impl core::fmt::Debug for ResourceResolver {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ResourceResolver")
            .field("sources", &self.sources.iter().map(|s| s.name()).collect::<Vec<_>>())
            .finish()
    }
}

impl ResourceResolver {
    /// A resolver with no sources.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a source.
    pub fn add(&mut self, source: crate::compat::Box<dyn ResourceSource>) {
        self.sources.push(source);
    }

    /// How many sources are registered.
    pub fn source_count(&self) -> usize {
        self.sources.len()
    }

    /// Resolves `id`, refusing an ambiguous identity.
    ///
    /// # Errors
    ///
    /// * [`ResolveError::NotFound`] — no source holds the id.
    /// * [`ResolveError::Ambiguous`] — more than one source holds it.
    /// * [`ResolveError::Failed`] — a source claimed the id but produced no bytes.
    pub fn resolve(&self, id: &ResourceId) -> Result<Vec<u8>, ResolveError> {
        let claimants: Vec<&crate::compat::Box<dyn ResourceSource>> =
            self.sources.iter().filter(|source| source.contains(id)).collect();
        match claimants.len() {
            0 => Err(ResolveError::NotFound { id: id.clone() }),
            1 => claimants[0].get(id).ok_or_else(|| ResolveError::Failed {
                id: id.clone(),
                reason: format!(
                    "source `{}` claimed the id but returned no bytes",
                    claimants[0].name()
                ),
            }),
            claimants => Err(ResolveError::Ambiguous { id: id.clone(), claimants }),
        }
    }

    /// Which source holds `id`, without producing bytes — a designer's "where is this from?".
    ///
    /// Returns the source names in registration order; more than one means the id is ambiguous, and
    /// the caller can show the conflict rather than guess.
    pub fn origins(&self, id: &ResourceId) -> Vec<&str> {
        self.sources
            .iter()
            .filter(|source| source.contains(id))
            .map(|source| source.name())
            .collect()
    }

    /// Resolves `id` from the **first** source that holds it, deliberately.
    ///
    /// The shadowing form, for a host that means one source to override another (a development
    /// override directory ahead of an embedded default). The name says so, because a caller reaching
    /// for this is choosing an order-dependent answer on purpose.
    pub fn resolve_preferring(&self, id: &ResourceId) -> Result<Vec<u8>, ResolveError> {
        for source in &self.sources {
            if source.contains(id) {
                return source.get(id).ok_or_else(|| ResolveError::Failed {
                    id: id.clone(),
                    reason: format!(
                        "source `{}` claimed the id but returned no bytes",
                        source.name()
                    ),
                });
            }
        }
        Err(ResolveError::NotFound { id: id.clone() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source_with(name: &str, entries: &[(&str, &[u8])]) -> EmbeddedSource {
        let mut source = EmbeddedSource::new(name);
        for (id, bytes) in entries {
            source.insert(ResourceId::parse(id).expect("a valid id"), bytes.to_vec());
        }
        source
    }

    /// Two spellings of one logical name must be the same id.
    #[test]
    fn equivalent_spellings_normalise_to_one_id() {
        let canonical = ResourceId::parse("icons/save.png").expect("valid");
        for spelling in
            ["./icons/save.png", "icons\\save.png", "icons//save.png", "/icons/save.png"]
        {
            assert_eq!(
                ResourceId::parse(spelling).expect("valid"),
                canonical,
                "`{spelling}` must normalise to the same id"
            );
        }
    }

    /// A name that escapes the root must be rejected, not resolved away.
    #[test]
    fn an_escaping_name_is_rejected() {
        assert_eq!(ResourceId::parse("../secret"), Err(ResourceIdError::EscapesRoot));
        assert_eq!(ResourceId::parse("a/../../b"), Err(ResourceIdError::EscapesRoot));
        assert_eq!(ResourceId::parse(""), Err(ResourceIdError::Empty));
        assert_eq!(ResourceId::parse("./."), Err(ResourceIdError::Empty));
    }

    /// `file_name` and `parent` must split a canonical id correctly.
    #[test]
    fn an_id_splits_into_a_name_and_a_parent() {
        let id = ResourceId::parse("audio/ui/click.wav").expect("valid");
        assert_eq!(id.file_name(), "click.wav");
        assert_eq!(id.parent(), "audio/ui");
        let top = ResourceId::parse("icon.png").expect("valid");
        assert_eq!(top.file_name(), "icon.png");
        assert_eq!(top.parent(), "");
    }

    /// A single source resolves its id to the bytes it holds.
    #[test]
    fn one_source_resolves_its_bytes() {
        let mut resolver = ResourceResolver::new();
        resolver.add(crate::compat::Box::new(source_with("embedded", &[("a/b.png", b"\x89PNG")])));
        let id = ResourceId::parse("a/b.png").expect("valid");
        assert_eq!(resolver.resolve(&id).expect("resolves"), b"\x89PNG".to_vec());
    }

    /// An id no source holds is `NotFound`, not an empty success.
    #[test]
    fn a_missing_id_is_not_found() {
        let mut resolver = ResourceResolver::new();
        resolver.add(crate::compat::Box::new(source_with("embedded", &[("a.png", b"x")])));
        let id = ResourceId::parse("missing.png").expect("valid");
        assert_eq!(resolver.resolve(&id), Err(ResolveError::NotFound { id }));
    }

    /// **The "重名" case**: two sources claiming one id must be reported ambiguous, not resolved to
    /// whichever happened to be registered first.
    #[test]
    fn a_duplicated_id_is_ambiguous() {
        let mut resolver = ResourceResolver::new();
        resolver.add(crate::compat::Box::new(source_with("first", &[("logo.png", b"first")])));
        resolver.add(crate::compat::Box::new(source_with("second", &[("logo.png", b"second")])));
        let id = ResourceId::parse("logo.png").expect("valid");
        assert_eq!(
            resolver.resolve(&id),
            Err(ResolveError::Ambiguous { id, claimants: 2 }),
            "a duplicated id must not resolve to an arbitrary claimant"
        );
    }

    /// `origins` must name every claimant, so a designer can show the conflict.
    #[test]
    fn origins_lists_every_claimant() {
        let mut resolver = ResourceResolver::new();
        resolver.add(crate::compat::Box::new(source_with("first", &[("logo.png", b"a")])));
        resolver.add(crate::compat::Box::new(source_with("second", &[("logo.png", b"b")])));
        let id = ResourceId::parse("logo.png").expect("valid");
        assert_eq!(resolver.origins(&id), vec!["first", "second"]);
    }

    /// The shadowing form must resolve the first source that holds the id — the deliberate,
    /// order-dependent lookup a development override uses.
    #[test]
    fn the_shadowing_form_resolves_the_first_claimant() {
        let mut resolver = ResourceResolver::new();
        resolver.add(crate::compat::Box::new(source_with("override", &[("theme.css", b"new")])));
        resolver.add(crate::compat::Box::new(source_with("default", &[("theme.css", b"old")])));
        let id = ResourceId::parse("theme.css").expect("valid");
        assert_eq!(resolver.resolve_preferring(&id).expect("resolves"), b"new".to_vec());
    }

    /// A moved resource must follow its **id**, not its old path: the same id resolves regardless of
    /// where the bytes now live, which is the property that makes runtime and package agree.
    #[test]
    fn an_id_resolves_independently_of_where_the_bytes_live() {
        let id = ResourceId::parse("icons/save.png").expect("valid");
        // Runtime: bytes come from one source.
        let mut runtime = ResourceResolver::new();
        runtime.add(crate::compat::Box::new(source_with("files", &[("icons/save.png", b"disk")])));
        // Package: the same id, a different source, different storage.
        let mut package = ResourceResolver::new();
        package
            .add(crate::compat::Box::new(source_with("embedded", &[("icons/save.png", b"bytes")])));
        // Both resolve the same id; only the bytes differ, which is the deployment's business.
        assert!(runtime.resolve(&id).is_ok());
        assert!(package.resolve(&id).is_ok());
        assert_eq!(runtime.origins(&id), vec!["files"]);
        assert_eq!(package.origins(&id), vec!["embedded"]);
    }

    /// An embedded source must list what it holds, sorted, for a completeness check.
    #[test]
    fn an_embedded_source_lists_its_ids_sorted() {
        let source = source_with("t", &[("b.png", b"b"), ("a.png", b"a"), ("c.png", b"c")]);
        let ids: Vec<String> = source.ids().iter().map(|id| id.as_str().into()).collect();
        assert_eq!(ids, vec!["a.png", "b.png", "c.png"]);
    }
}
