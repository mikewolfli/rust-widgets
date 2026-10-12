// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Lightweight, already-resolved icon content for widget presentation.

use crate::widget::display_widgets::icon::{IconData, IconName};

/// Persistable description of an icon source.
///
/// Unlike [`IconSource`], this contains no borrowed geometry or runtime pixel handle and can
/// therefore cross a document boundary. Resource descriptors must be resolved by an explicit
/// application context before they can be presented by a widget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IconSpec {
    /// A built-in token shipped by this crate.
    Builtin(IconName),
    /// A project resource whose bytes are resolved by the host.
    Resource {
        /// Stable project-local resource identity.
        id: alloc::string::String,
        /// Whether the resource is a tintable monochrome mask or keeps its original colours.
        color_mode: IconColorMode,
    },
}

/// Color treatment declared for a persisted resource icon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconColorMode {
    /// Treat the resource as a monochrome mask that may be tinted by the widget.
    Monochrome,
    /// Preserve the resource's own colors.
    Original,
}

/// An icon source that can be used without a resource resolver.
///
/// `Builtin` uses the crate's token and preserves its feature-dependent fallback. `StaticGeometry`
/// borrows caller-owned static path data directly and does not occupy the legacy name registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconSource {
    /// One of the crate's built-in icon names.
    Builtin(IconName),
    /// Static path geometry on the grid described by its [`IconData`].
    StaticGeometry(&'static IconData),
}

/// Why an icon source could not be assigned to a collection item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconItemError {
    /// The requested item index does not exist.
    IndexOutOfBounds {
        /// The index the caller requested.
        index: usize,
        /// Number of items in the collection.
        len: usize,
    },
    /// No menu item with the requested stable identifier exists.
    ItemNotFound {
        /// The item identifier the caller requested.
        id: u64,
    },
    /// More than one menu item uses the requested identifier.
    AmbiguousItemId {
        /// The non-unique item identifier.
        id: u64,
    },
}

impl IconSource {
    /// Creates a source for a built-in token.
    pub const fn builtin(name: IconName) -> Self {
        Self::Builtin(name)
    }

    /// Creates a source borrowing static geometry.
    pub const fn static_geometry(data: &'static IconData) -> Self {
        Self::StaticGeometry(data)
    }
}

impl IconSpec {
    /// Resolves a persisted built-in source without any external services.
    ///
    /// A resource descriptor intentionally returns `None`: resolving it requires an explicit
    /// project/resource context and must not turn a resource ID into an assumed file path.
    pub fn builtin_source(&self) -> Option<IconSource> {
        match self {
            Self::Builtin(name) => Some(IconSource::Builtin(*name)),
            Self::Resource { .. } => None,
        }
    }
}
