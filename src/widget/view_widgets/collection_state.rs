// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Empty- and error-state hooks shared by the collection views (RW-17).
//!
//! # Why one state object and one trait
//!
//! Every collection control can be handed no rows, and most of them can be handed a failure
//! instead of rows. Before this module each control had nowhere to put either fact, so a host
//! that owned the text had to draw its own caption on top of the control's rectangle — which
//! meant the caption did not move with the control, did not use its font and did not survive a
//! theme change.
//!
//! [`CollectionPlaceholder`] is the two strings; [`CollectionState`] is the small set of
//! accessors a control exposes; [`paint_collection_placeholder`] is the one paint path. The
//! trait keeps the per-control code to two one-line accessors, and the shared paint keeps the
//! five views from drifting apart in font, colour or alignment.

use crate::core::{Color, Font, HorizontalAlignment, Rect};
use crate::render::RenderContext;

/// The centred text a collection view shows when it has nothing to show.
///
/// The error text wins over the empty text: a view that failed to load is not "empty", and a host
/// that has reported a failure must not have it replaced by a neutral caption.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CollectionPlaceholder {
    empty_text: Option<String>,
    error_text: Option<String>,
}

impl CollectionPlaceholder {
    /// An empty placeholder with no text of either kind.
    pub fn new() -> Self {
        Self::default()
    }

    /// The empty-state caption, when one was set.
    pub fn empty_text(&self) -> Option<&str> {
        self.empty_text.as_deref()
    }

    /// Sets the empty-state caption; an empty string clears it. Returns whether it changed.
    pub fn set_empty_text(&mut self, text: impl Into<String>) -> bool {
        let text = text.into();
        let next = if text.is_empty() { None } else { Some(text) };
        if self.empty_text == next {
            return false;
        }
        self.empty_text = next;
        true
    }

    /// Clears the empty-state caption. Returns whether one was set.
    pub fn clear_empty_text(&mut self) -> bool {
        self.empty_text.take().is_some()
    }

    /// The error-state caption, when one was set.
    pub fn error_text(&self) -> Option<&str> {
        self.error_text.as_deref()
    }

    /// Sets the error-state caption; an empty string clears it. Returns whether it changed.
    pub fn set_error_text(&mut self, text: impl Into<String>) -> bool {
        let text = text.into();
        let next = if text.is_empty() { None } else { Some(text) };
        if self.error_text == next {
            return false;
        }
        self.error_text = next;
        true
    }

    /// Clears the error-state caption. Returns whether one was set.
    pub fn clear_error_text(&mut self) -> bool {
        self.error_text.take().is_some()
    }

    /// The caption that should be painted, if any: the error text, else the empty text.
    pub fn text(&self) -> Option<&str> {
        self.error_text.as_deref().or(self.empty_text.as_deref())
    }

    /// Whether the placeholder is in its error state.
    pub fn is_error(&self) -> bool {
        self.error_text.is_some()
    }

    /// Whether neither caption is set.
    pub fn is_empty(&self) -> bool {
        self.empty_text.is_none() && self.error_text.is_none()
    }
}

/// The accessors a collection view publishes for its placeholder (RW-17).
///
/// Implemented by `TableWidget`, `ListView`, `TreeView`, `VirtualTable` and `VirtualList`. Each
/// implementation is two field accessors; the trait supplies the six public methods and the
/// redraw request, so the controls cannot disagree about what "no text" means (an empty string
/// clears the caption in every one of them).
pub trait CollectionState: crate::widget::Widget {
    /// The control's placeholder state.
    fn collection_placeholder(&self) -> &CollectionPlaceholder;
    /// The control's placeholder state, mutably.
    fn collection_placeholder_mut(&mut self) -> &mut CollectionPlaceholder;

    /// The empty-state caption, when one is set.
    fn empty_text(&self) -> Option<&str> {
        self.collection_placeholder().empty_text()
    }

    /// Sets the empty-state caption; an empty string clears it.
    fn set_empty_text(&mut self, text: impl Into<String>) {
        if self.collection_placeholder_mut().set_empty_text(text) {
            self.base().request_redraw();
        }
    }

    /// Clears the empty-state caption.
    fn clear_empty_text(&mut self) {
        if self.collection_placeholder_mut().clear_empty_text() {
            self.base().request_redraw();
        }
    }

    /// The error-state caption, when one is set.
    fn error_text(&self) -> Option<&str> {
        self.collection_placeholder().error_text()
    }

    /// Sets the error-state caption; an empty string clears it.
    fn set_error_text(&mut self, text: impl Into<String>) {
        if self.collection_placeholder_mut().set_error_text(text) {
            self.base().request_redraw();
        }
    }

    /// Clears the error-state caption.
    fn clear_error_text(&mut self) {
        if self.collection_placeholder_mut().clear_error_text() {
            self.base().request_redraw();
        }
    }
}

/// Paints the placeholder's caption centred in `band`, returning whether anything was painted.
///
/// The line box comes from [`RenderContext::text_line`] so the glyphs sit on the band's middle
/// line rather than starting there; the error state reads the crate's error colour so a failure
/// is distinguishable from "there is nothing here yet" without a second icon.
pub fn paint_collection_placeholder(
    context: &mut RenderContext,
    band: Rect,
    placeholder: &CollectionPlaceholder,
    ink: Color,
    font: &Font,
) -> bool {
    let Some(text) = placeholder.text() else {
        return false;
    };
    if band.width == 0 || band.height == 0 {
        return false;
    }
    let color = if placeholder.is_error() { Color::ERROR } else { ink.with_alpha_f32(0.65) };
    context.draw_text_fitted_ellipsis(
        context.text_line(band, font),
        text,
        font,
        color,
        HorizontalAlignment::Center,
    );
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_strings_clear_the_captions() {
        let mut placeholder = CollectionPlaceholder::new();
        assert!(placeholder.is_empty());

        assert!(placeholder.set_empty_text("No rows"));
        assert_eq!(placeholder.empty_text(), Some("No rows"));
        assert_eq!(placeholder.text(), Some("No rows"));
        assert!(!placeholder.set_empty_text("No rows"), "setting the same text is not a change");

        assert!(placeholder.clear_empty_text());
        assert!(!placeholder.clear_empty_text(), "clearing twice is not a change");
        assert!(placeholder.is_empty());

        assert!(placeholder.set_error_text("Load failed"));
        assert!(placeholder.is_error());
        assert_eq!(placeholder.text(), Some("Load failed"), "the error caption wins");
        assert!(placeholder.set_error_text(""));
        assert!(!placeholder.is_error());
    }

    #[test]
    fn the_error_caption_takes_precedence_over_the_empty_caption() {
        let mut placeholder = CollectionPlaceholder::new();
        placeholder.set_empty_text("No rows");
        placeholder.set_error_text("Load failed");
        assert_eq!(placeholder.text(), Some("Load failed"));
        placeholder.clear_error_text();
        assert_eq!(placeholder.text(), Some("No rows"));
    }
}
