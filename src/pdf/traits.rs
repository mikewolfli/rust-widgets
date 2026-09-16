// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! PDF page and document traits.
//!
//! # Coordinate system and units
//!
//! All geometry is in **PDF points**, where one point is 1/72 inch, and page
//! sizes are given as [`Size`] in the same unit (so a US Letter page is
//! `612 x 792`).
//!
//! The y axis of the **public API** follows the rest of the crate: the origin is
//! the **top-left** of the page and y grows downwards. Implementations are
//! responsible for flipping this into PDF's own **bottom-left** origin when they
//! emit content-stream operators, so callers of [`PdfPage`] never need to think
//! about PDF's native convention. `PdfPageImpl` does this via `to_pdf_y(y,
//! page_height)`, which maps `y = 0` to the top edge and `y = height` to the
//! bottom.
//!
//! # Contract
//!
//! Implementations are expected to accumulate drawing operations and expose them
//! through [`PdfPage::content`] in the order they were issued; the traits make no
//! claim about *when* work is rasterised or written out, only that the page
//! records the sequence faithfully. Page indices are `u32` and **0-based**
//! throughout, matching a page's position in the document.

use crate::core::{Color, Rect, Size};
use crate::pdf::metadata::PdfMetadata;
use crate::pdf::types::PdfFormField;
use crate::pdf::PdfSecurity;

/// PDF page trait
///
/// A single page's drawing surface plus the interactive form fields defined on
/// it. Coordinates are in points with a top-left origin; see the [module
/// documentation](self) for the full convention.
///
/// Drawing methods do not fail: out-of-range geometry is the implementation's
/// responsibility to clip or ignore, and nothing is returned to the caller.
/// Implementations are expected to be append-only, so repeated draws accumulate
/// on the page rather than replacing one another.
pub trait PdfPage {
    /// Returns the page size in points.
    ///
    /// This is the size the y-axis flip is computed against, so changing it
    /// affects where subsequently drawn content lands.
    fn size(&self) -> Size;
    /// Sets the page size in points.
    ///
    /// Existing content is not re-laid-out or rescaled; only the reference
    /// height used to flip subsequent coordinates changes.
    fn set_size(&mut self, size: Size);
    /// Draws `text` with its **baseline start** at `(x, y)` in points.
    ///
    /// `font_size` is in points and `color` is applied as the fill colour. The
    /// implementation renders with the page's configured font resource and
    /// expects a standard sans-serif vertical metric; it does not shape the text
    /// or apply the glyph layout used by the widget renderer, so complex scripts
    /// and precise advance widths are not guaranteed. `text` is escaped for the
    /// PDF literal-string syntax, so parentheses and backslashes are handled.
    fn draw_text(&mut self, text: &str, x: f32, y: f32, font_size: f32, color: Color);
    /// Strokes a straight line from `(x1, y1)` to `(x2, y2)`, all in points.
    ///
    /// `width` is the stroke width in points. Endpoints are specified in the
    /// public top-left-origin space.
    fn draw_line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, width: f32, color: Color);
    /// Strokes the outline of `rect` with a `width`-point stroke in `color`.
    ///
    /// `rect` uses the public top-left-origin space. The stroke is centred on
    /// the rectangle edges, so it extends `width / 2` points outside `rect`.
    fn draw_rect(&mut self, rect: Rect, width: f32, color: Color);
    /// Fills `rect` with `color`.
    ///
    /// `rect` uses the public top-left-origin space, and its `x`, `y`, `width`,
    /// and `height` are the rectangle's extent in points. A zero-width or
    /// zero-height rectangle encloses no area and paints nothing.
    fn fill_rect(&mut self, rect: Rect, color: Color);
    /// Draws `image` into `rect`, both in the public top-left-origin space.
    ///
    /// `image` is a raw pixel payload — not an encoded file — which the
    /// implementation normalises to RGB (RGBA input has its alpha dropped, since
    /// the PDF image operators used here carry no alpha channel) and lays out as
    /// `rect.width x rect.height` pixels. If the payload's length does not match
    /// the rectangle's pixel count, the image is treated as having been supplied
    /// at its own dimensions. An empty payload, or a rectangle with zero width or
    /// height, is ignored.
    fn draw_image(&mut self, image: &[u8], rect: Rect);
    /// Adds a single-line text field named `name` occupying `rect`.
    ///
    /// `default_text` is the field's initial value. Field names identify the
    /// field within the page's collection, so adding a name that already exists
    /// **replaces** the previous definition rather than adding a second field.
    /// `rect` is in the public top-left-origin space.
    fn add_text_field(&mut self, name: &str, rect: Rect, default_text: &str);
    /// Adds a checkbox named `name` occupying `rect`, initially `checked` or not.
    ///
    /// As with [`PdfPage::add_text_field`], reusing a name replaces the existing
    /// definition.
    fn add_checkbox(&mut self, name: &str, rect: Rect, checked: bool);
    /// Adds a push button named `name` occupying `rect` with the label `text`.
    ///
    /// As with [`PdfPage::add_text_field`], reusing a name replaces the existing
    /// definition. Buttons have no value of their own, so only the label is
    /// stored.
    fn add_button(&mut self, name: &str, rect: Rect, text: &str);
    /// Returns the page's accumulated content stream as encoded PDF operators.
    ///
    /// The bytes are the raw operator text that would be written into the page
    /// object, not a complete PDF file; serialising a document is
    /// [`PdfDocument::to_bytes`]' job. Implementations return an owned copy, so
    /// the caller may hold it while continuing to draw.
    fn content(&self) -> Vec<u8>;
    /// Returns the interactive form fields defined on this page.
    ///
    /// The returned vector is owned and the fields are ordered by name, so the
    /// result is stable across calls regardless of the order fields were added
    /// in.
    fn form_fields(&self) -> Vec<PdfFormField>;
}

/// PDF document trait
///
/// Owns an ordered set of pages, the document-level metadata and security
/// settings, and the output path. Page indices are **0-based** and always refer
/// to the current order, so an index is invalidated by
/// [`PdfDocument::insert_page`], [`PdfDocument::remove_page`], and
/// [`PdfDocument::reorder_pages`].
///
/// Implementations are expected to keep at least one page at all times, since a
/// zero-page PDF is not representable; see [`PdfDocument::remove_page`].
pub trait PdfDocument {
    /// Returns the number of pages currently in the document.
    fn page_count(&self) -> u32;
    /// Borrows the page at `index`, or `None` when `index` is out of range.
    ///
    /// The borrow is mutable because drawing mutates the page in place. Rust's
    /// borrow rules prevent holding two such borrows at once, so pages must be
    /// drawn one at a time.
    fn get_page(&mut self, index: u32) -> Option<&mut dyn PdfPage>;
    /// Appends a page of `size` points and returns its index.
    ///
    /// The new page is empty apart from the document's default font resource,
    /// and its index is always the previous page count.
    fn add_page(&mut self, size: Size) -> u32;
    /// Inserts a page of `size` points at `index`, returning the index it
    /// occupies.
    ///
    /// Pages at and after `index` shift up by one, which invalidates any index
    /// the caller was holding. An `index` beyond the end is clamped: the page is
    /// appended and the index of the last page is returned.
    fn insert_page(&mut self, index: u32, size: Size) -> u32;
    /// Removes the page at `index`.
    ///
    /// Returns `true` if a page was removed, `false` if `index` was out of range
    /// **or** if it would leave the document with no pages at all — the last
    /// remaining page cannot be removed.
    fn remove_page(&mut self, index: u32) -> bool;
    /// Reorders the pages so that the page currently at `new_order[i]` becomes
    /// page `i`.
    ///
    /// `new_order` must therefore be a permutation of `0..page_count` with
    /// exactly one entry per page; any other input is rejected. Returns `true`
    /// on success and `false` if the length is wrong or the values are not a
    /// permutation, in which case the existing order is untouched. Page objects
    /// move with their content, so drawn output travels with the page.
    fn reorder_pages(&mut self, new_order: &[u32]) -> bool;
    /// Borrows the document metadata (title, author, and so on).
    fn metadata(&self) -> &PdfMetadata;
    /// Replaces the document metadata wholesale.
    fn set_metadata(&mut self, metadata: PdfMetadata);
    /// Borrows the document's security settings.
    ///
    /// Security is stored and serialised; the returned value describes the
    /// configured state, not the encryption applied to the bytes returned by
    /// [`PdfDocument::to_bytes`].
    fn security(&self) -> &PdfSecurity;
    /// Replaces the document's security settings wholesale.
    fn set_security(&mut self, security: PdfSecurity);
    /// Enables or disables the automatic page-number footer.
    ///
    /// Numbering is off by default and only affects documents written from this
    /// point on; pages already written out are unaffected. The footer's text and
    /// placement come from [`PdfDocument::set_page_numbering_format`] and
    /// [`PdfDocument::set_page_numbering_layout`].
    fn set_page_numbering_enabled(&mut self, enabled: bool);
    /// Configures the page-number footer's label and starting number.
    ///
    /// `prefix` is the text placed before the number; an empty or
    /// whitespace-only value falls back to `"Page"`. `start_at` is the number
    /// given to the **first** page, and is clamped to a minimum of `1`, so page
    /// numbering always starts at one or above.
    fn set_page_numbering_format(&mut self, prefix: &str, start_at: u32);
    /// Configures the page-number footer's placement and text size.
    ///
    /// `right_margin` and `bottom_margin` are distances in points from the
    /// page's right and bottom edges respectively, each clamped to a minimum of
    /// `0.0`. `font_size` is in points and is clamped to a minimum of `6.0`, so
    /// the footer remains legible.
    fn set_page_numbering_layout(&mut self, right_margin: f32, bottom_margin: f32, font_size: f32);
    /// Serialises the document and writes it to the filesystem at `path`.
    ///
    /// The write is not atomic: an existing file at `path` is truncated before
    /// the new bytes are written, so a failure part-way through can leave a
    /// partial file behind. Parent directories are not created. Errors are
    /// reported as [`std::io::Error`] from either the serialisation or the write.
    fn save(&self, path: &str) -> Result<(), std::io::Error>;
    /// Serialises the whole document into an owned byte buffer.
    ///
    /// The result is a complete PDF file image, suitable for writing to disk or
    /// sending over a network, and is the same content [`PdfDocument::save`]
    /// writes. Serialisation can fail, for example when a font must be embedded
    /// and cannot be read.
    fn to_bytes(&self) -> Result<Vec<u8>, std::io::Error>;
}
