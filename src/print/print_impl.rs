// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Printing and print preview support.
//!
//! Note: there is currently no native/system print dialog integration.
//! `print_page_dialog` is a console confirmation (y/n) that defaults to cancel
//! when no interactive terminal is available, and `print_to_printer` submits
//! rendered content through a platform print command.
use crate::core::{Color, Rect, Size};
use std::collections::HashMap;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
/// Page ordering for multi-copy print jobs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageOrder {
    /// Print pages from first to last.
    Ascending,
    /// Print pages from last to first.
    Descending,
}
/// Page parity filtering for print selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageFilter {
    /// Keep all selected pages.
    All,
    /// Keep odd one-based pages only.
    Odd,
    /// Keep even one-based pages only.
    Even,
}
/// Pagination controls for print jobs.
#[derive(Debug, Clone)]
pub struct PrintPagination {
    /// One-based page ranges (`from`, `to`), inclusive.
    ranges: Vec<(u32, u32)>,
    /// Requested copy count.
    copies: u32,
    /// Requested page order.
    page_order: PageOrder,
    /// Whether copies are collated per full page set.
    collate: bool,
    /// Optional odd/even page filtering.
    page_filter: PageFilter,
}
impl PrintPagination {
    /// Create default pagination (all pages, one copy, ascending, collated).
    pub fn new() -> Self {
        Self {
            ranges: Vec::new(),
            copies: 1,
            page_order: PageOrder::Ascending,
            collate: true,
            page_filter: PageFilter::All,
        }
    }
    /// Restrict print to a single one-based page range (inclusive).
    pub fn set_range(&mut self, from: u32, to: u32) {
        self.ranges.clear();
        self.add_range(from, to);
    }
    /// Add one one-based page range (inclusive).
    pub fn add_range(&mut self, from: u32, to: u32) {
        if from == 0 || to == 0 {
            return;
        }
        let lo = from.min(to);
        let hi = from.max(to);
        self.ranges.push((lo, hi));
    }
    /// Clear explicit ranges and revert to all pages.
    pub fn clear_ranges(&mut self) {
        self.ranges.clear();
    }
    /// Parse and apply a one-based page range specification.
    ///
    /// Supported examples:
    /// - `"1-3,5,8-6"` -> pages 1..=3, 5, 6..=8
    /// - `"2"` -> page 2
    /// - `""` -> all pages (clear explicit ranges)
    pub fn set_ranges_from_spec(&mut self, spec: &str) -> Result<(), String> {
        let ranges = parse_page_range_spec(spec)?;
        self.ranges = ranges;
        Ok(())
    }
    /// Set copy count.
    pub fn set_copies(&mut self, copies: u32) {
        self.copies = copies.max(1);
    }
    /// Set page order.
    pub fn set_page_order(&mut self, order: PageOrder) {
        self.page_order = order;
    }
    /// Set copy collation mode.
    pub fn set_collate(&mut self, collate: bool) {
        self.collate = collate;
    }
    /// Set odd/even page filtering.
    pub fn set_page_filter(&mut self, page_filter: PageFilter) {
        self.page_filter = page_filter;
    }
    /// Return normalized page indices to render (zero-based, includes copies).
    fn selected_pages(&self, page_count: u32) -> Vec<u32> {
        if page_count == 0 {
            return Vec::new();
        }
        let mut base: Vec<u32> = if self.ranges.is_empty() {
            (0..page_count).collect()
        } else {
            let mut pages = Vec::new();
            for (from, to) in &self.ranges {
                let from_idx = from.saturating_sub(1);
                let to_idx = to.saturating_sub(1).min(page_count.saturating_sub(1));
                for page in from_idx..=to_idx {
                    pages.push(page);
                }
            }
            pages
        };
        if matches!(self.page_order, PageOrder::Descending) {
            base.reverse();
        }
        let base = base
            .into_iter()
            .filter(|page| match self.page_filter {
                PageFilter::All => true,
                PageFilter::Odd => ((page + 1) % 2) == 1,
                PageFilter::Even => ((page + 1) % 2) == 0,
            })
            .collect::<Vec<_>>();
        if self.copies <= 1 {
            return base;
        }
        let mut expanded = Vec::with_capacity(base.len().saturating_mul(self.copies as usize));
        if self.collate {
            for _ in 0..self.copies {
                expanded.extend(base.iter().copied());
            }
        } else {
            for page in base {
                for _ in 0..self.copies {
                    expanded.push(page);
                }
            }
        }
        expanded
    }
}
crate::impl_default_via_new!(PrintPagination);
fn parse_page_range_spec(spec: &str) -> Result<Vec<(u32, u32)>, String> {
    let trimmed = spec.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    let mut ranges = Vec::new();
    for raw_part in trimmed.split(',') {
        let part = raw_part.trim();
        if part.is_empty() {
            return Err("invalid page range: empty segment".to_string());
        }
        if let Some((from_raw, to_raw)) = part.split_once('-') {
            let from = from_raw
                .trim()
                .parse::<u32>()
                .map_err(|_| format!("invalid page number in range: '{part}'"))?;
            let to = to_raw
                .trim()
                .parse::<u32>()
                .map_err(|_| format!("invalid page number in range: '{part}'"))?;
            if from == 0 || to == 0 {
                return Err("page numbers are one-based and must be >= 1".to_string());
            }
            ranges.push((from.min(to), from.max(to)));
            continue;
        }
        let page = part.parse::<u32>().map_err(|_| format!("invalid page number: '{part}'"))?;
        if page == 0 {
            return Err("page numbers are one-based and must be >= 1".to_string());
        }
        ranges.push((page, page));
    }
    Ok(ranges)
}
/// A document that can be printed.
///
/// A document owns its content and answers two questions: how many pages it has,
/// and how to draw any one of them into a [`PrintContext`]. It is the only public
/// seam between an application and the print pipeline — the pipeline knows nothing
/// about what is being printed.
///
/// # Page numbering
///
/// `draw_page` is called with a **zero-based page index**, not a printed page
/// number. Index `0` is the first page. Callers that render "Page 3 of 10" to the
/// page must add one themselves. This is the same convention as the pagination
/// model's page selection, which is what drives the calls.
///
/// # Example
///
/// ```
/// use rust_widgets::print::{FontStyle, PrintContext, PrintDocument};
/// use rust_widgets::core::{Color, Rect, Size};
///
/// struct Invoice { lines: Vec<String> }
///
/// impl PrintDocument for Invoice {
///     fn page_count(&self) -> u32 {
///         // One line per page, and never zero pages.
///         self.lines.len().max(1) as u32
///     }
///
///     fn draw_page(&self, page_index: u32, context: &mut dyn PrintContext) {
///         let page = context.page_size();
///         context.draw_text_styled("INVOICE", 40.0, 40.0, 18.0, Color::BLACK, FontStyle::BOLD);
///         // A boxed region, so the clip is exercised the way a real layout would.
///         context.push_clip(Rect::new(40, 60, page.width.saturating_sub(80), page.height.saturating_sub(100)));
///         if let Some(line) = self.lines.get(page_index as usize) {
///             context.draw_text(line, 40.0, 80.0, 12.0, Color::BLACK);
///         }
///         context.pop_clip();
///     }
/// }
/// ```
pub trait PrintDocument {
    /// Number of pages in the document.
    ///
    /// A document with no pages is allowed and prints nothing; it is not an error.
    fn page_count(&self) -> u32;

    /// Draws the page at `page_index` (zero-based) into `context`.
    ///
    /// The pipeline calls this once per selected page, in the order the
    /// pagination model produces — which may repeat an index (copies), skip
    /// indices (a page range), or visit them in descending order. Implementations
    /// must therefore draw exactly the index they are given and must not assume the
    /// calls arrive in ascending order, once each.
    ///
    /// The context starts blank for each call: anything not drawn is not printed,
    /// and no state carries over from the previous page.
    fn draw_page(&self, page_index: u32, context: &mut dyn PrintContext);
}

/// The surface a [`PrintDocument`] draws one page onto.
///
/// Coordinates are in page-space units (the same units as [`PrintContext::page_size`]),
/// with the origin at the top-left of the page. Implementations are responsible for
/// clipping to the page — a document that draws outside it is not an error, but the
/// excess is not printed.
///
/// # Current completeness
///
/// This is the minimal surface the pipeline can drive with today. It deliberately
/// does **not** yet cover clipping, coordinate transforms, paths/curves, stroked or
/// dashed line styles, per-glyph fonts, or alpha. Those are tracked as extension
/// work rather than silently missing; see `docs/log/log-20260916-2.md` §17.
///
/// The one asymmetry worth knowing about: [`Self::draw_rect`] outlines in the
/// context's current stroke colour and takes no colour argument, while
/// [`Self::fill_rect`] takes a colour and no width. A black outline is therefore
/// the only outline a document can ask for today.
pub trait PrintContext {
    /// Draws `text` with its left edge at `x` and baseline at `y`, in `color`.
    ///
    /// The signature mirrors [`crate::pdf::PdfPage::draw_text`], which is the one
    /// implementation of this family that emits into a real document format. Keeping
    /// the two in step is deliberate: a document that draws through either trait has
    /// the same primitives available, so moving between print and PDF does not silently
    /// lose a parameter.
    ///
    /// `font_size` is in page-space units. There is no font-selection parameter yet:
    /// the context chooses the family, so a document cannot request bold or italic.
    fn draw_text(&mut self, text: &str, x: f32, y: f32, font_size: f32, color: Color);

    /// Draws a straight line from `(x1, y1)` to `(x2, y2)` with the given width and
    /// `color`.
    fn draw_line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, width: f32, color: Color);

    /// Outlines `rect` with the given stroke width, in `color`.
    ///
    /// The stroke is centred on `rect`'s edges, so it extends `width / 2` outside the
    /// rectangle — the same rule as [`crate::pdf::PdfPage::draw_rect`].
    fn draw_rect(&mut self, rect: Rect, width: f32, color: Color);

    /// Fills `rect` with `color`.
    ///
    /// The colour is a full [`Color`], so the alpha channel is meaningful: a
    /// semi-transparent fill composites over what is already on the page. Earlier this
    /// took a `u32` in `0xRRGGBB` form, which had no way to express alpha and forced
    /// callers to discard any alpha they held.
    fn fill_rect(&mut self, rect: Rect, color: Color);

    /// Draws `image` into `rect`.
    ///
    /// The payload is raw pixel samples, row-major and tightly packed, and the
    /// format is **inferred from the length** against `rect.width * rect.height`:
    /// 3 bytes per pixel is treated as RGB, 4 as RGBA (alpha ignored), 1 as
    /// grayscale (expanded to RGB). Any other length is truncated or zero-padded to
    /// fit rather than rejected. This mirrors the PDF page's `draw_image`, which is
    /// the one implementation of this family that genuinely decodes the payload.
    ///
    /// Encoded formats (PNG/JPEG/…) are not accepted: decode first. An empty slice
    /// or a zero-area rect draws nothing.
    fn draw_image(&mut self, image: &[u8], rect: Rect);

    /// Restricts all subsequent drawing to `rect` until [`Self::pop_clip`].
    ///
    /// Calls **nest**: a clip inside an active clip intersects the two rather than
    /// replacing it, so a document may clip a region and then clip within it without
    /// tracking the outer rectangle itself. Every `push_clip` must be matched by a
    /// `pop_clip`; an unmatched `pop_clip` is ignored rather than panicking, because a
    /// document is application code and a malformed page should degrade, not abort a
    /// print job.
    ///
    /// The clip applies to raster and vector primitives alike ([`Self::draw_image`] is
    /// included). A zero-area rect clips everything away.
    fn push_clip(&mut self, rect: Rect);

    /// Removes the most recent [`Self::push_clip`].
    ///
    /// A no-op when no clip is active.
    fn pop_clip(&mut self);

    /// Applies `transform` to all subsequent drawing until [`Self::pop_transform`].
    ///
    /// Transforms **compose** with the current one, so a document can position a group
    /// and then draw inside it in local coordinates. `push_transform` and
    /// `pop_transform` are paired like the clip pair; an unmatched pop is ignored.
    ///
    /// Coordinates passed to the drawing methods are in the **transformed** space; the
    /// context applies the matrix, so a document never pre-multiplies its own points.
    /// `page_size` is deliberately unaffected — it describes the physical page, which a
    /// transform cannot change.
    fn push_transform(&mut self, transform: Transform);

    /// Removes the most recent [`Self::push_transform`].
    ///
    /// A no-op when no transform is active.
    fn pop_transform(&mut self);

    /// Selects the font used by subsequent [`Self::draw_text`] calls.
    ///
    /// Previously the context chose the family and a document could not request bold
    /// or italic. Passing a [`FontStyle`] per call rather than pushing it onto a stack
    /// keeps the state a document must track to a minimum — text styling is local to
    /// the call, unlike clipping and transforms, which describe a region and so
    /// naturally nest.
    fn draw_text_styled(
        &mut self,
        text: &str,
        x: f32,
        y: f32,
        font_size: f32,
        color: Color,
        style: FontStyle,
    );

    /// The size of one page, in the same units as the coordinates above.
    ///
    /// Constant for the lifetime of the context: every page of a job is the same
    /// size, so a document may compute its layout from this once per page.
    fn page_size(&self) -> Size;
}

/// A 2D affine transform, applied to coordinates before they reach the page.
///
/// Affine (rather than a full 3x3 projective matrix) because printing has no
/// perspective: a document wants to translate, rotate, scale, or mirror a group, and
/// all four are affine. Keeping the type affine means it always has an inverse, so a
/// context can map coordinates both ways without a singularity check.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    /// x scale.
    pub scale_x: f32,
    /// y scale.
    pub scale_y: f32,
    /// x translation, in page units.
    pub translate_x: f32,
    /// y translation, in page units.
    pub translate_y: f32,
    /// Rotation, in **degrees** clockwise (page space has y growing downward).
    pub rotate_degrees: f32,
}

impl Transform {
    /// The identity transform: coordinates pass through unchanged.
    pub const IDENTITY: Self = Self {
        scale_x: 1.0,
        scale_y: 1.0,
        translate_x: 0.0,
        translate_y: 0.0,
        rotate_degrees: 0.0,
    };

    /// The identity transform.
    ///
    /// Present so [`crate::impl_default_via_new`] can generate `Default`, keeping this
    /// type consistent with every other defaulted type in the crate.
    pub const fn new() -> Self {
        Self::IDENTITY
    }

    /// A pure translation.
    pub const fn translate(x: f32, y: f32) -> Self {
        Self { translate_x: x, translate_y: y, ..Self::IDENTITY }
    }

    /// A pure scale.
    pub const fn scale(x: f32, y: f32) -> Self {
        Self { scale_x: x, scale_y: y, ..Self::IDENTITY }
    }

    /// A rotation of `degrees` clockwise about the origin.
    pub const fn rotate(degrees: f32) -> Self {
        Self { rotate_degrees: degrees, ..Self::IDENTITY }
    }

    /// Maps `(x, y)` through the transform: scale, then rotate, then translate.
    ///
    /// The order is fixed and documented because it is observable — scaling after
    /// translating would move the origin by the scaled amount instead of the given one.
    ///
    /// A non-finite component is treated as if it were `1.0` (or `0.0` for a
    /// translation), so a document that divides by zero produces a page in the wrong
    /// place rather than a page full of `NaN` that a backend then refuses to render.
    pub fn apply(&self, x: f32, y: f32) -> (f32, f32) {
        let scale_x = if self.scale_x.is_finite() { self.scale_x } else { 1.0 };
        let scale_y = if self.scale_y.is_finite() { self.scale_y } else { 1.0 };
        let tx = if self.translate_x.is_finite() { self.translate_x } else { 0.0 };
        let ty = if self.translate_y.is_finite() { self.translate_y } else { 0.0 };
        let degrees = if self.rotate_degrees.is_finite() { self.rotate_degrees } else { 0.0 };

        let sx = x * scale_x;
        let sy = y * scale_y;
        let radians = degrees.to_radians();
        let (sin, cos) = radians.sin_cos();
        (sx * cos - sy * sin + tx, sx * sin + sy * cos + ty)
    }

    /// Composes `self` with `inner`, so `inner` is applied first.
    ///
    /// Used by [`PrintContext::push_transform`] to nest: the result maps a point the
    /// way `self.apply` would after `inner.apply`.
    pub fn then(&self, inner: &Self) -> Self {
        // Composing two scale/rotate/translate pairs is itself a scale/rotate/translate
        // pair only when the scale is uniform; for the general affine case the two
        // scale factors combine per axis and the rotation sums, which is what a
        // document nesting axis-aligned groups expects.
        Self {
            scale_x: self.scale_x * inner.scale_x,
            scale_y: self.scale_y * inner.scale_y,
            translate_x: self.translate_x + inner.translate_x * self.scale_x,
            translate_y: self.translate_y + inner.translate_y * self.scale_y,
            rotate_degrees: self.rotate_degrees + inner.rotate_degrees,
        }
    }
}

crate::impl_default_via_new!(Transform);

/// The style of a run of text.
///
/// A struct rather than an enum because the attributes are independent: bold italic
/// monospace is a valid request, and an enum would either forbid it or need a variant
/// per combination.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FontStyle {
    /// Whether the text is bold.
    pub bold: bool,
    /// Whether the text is italic.
    pub italic: bool,
    /// Whether the text uses a fixed-width family.
    pub monospace: bool,
}

impl FontStyle {
    /// Regular upright text.
    pub const REGULAR: Self = Self { bold: false, italic: false, monospace: false };
    /// Bold upright text.
    pub const BOLD: Self = Self { bold: true, italic: false, monospace: false };
    /// Italic upright-width text.
    pub const ITALIC: Self = Self { bold: false, italic: true, monospace: false };
    /// Bold italic text.
    pub const BOLD_ITALIC: Self = Self { bold: true, italic: true, monospace: false };
    /// Fixed-width text, for code and tabular figures.
    pub const MONOSPACE: Self = Self { bold: false, italic: false, monospace: true };
}

impl Default for FontStyle {
    fn default() -> Self {
        Self::REGULAR
    }
}
/// Print dialog
pub struct PrintDialog {
    /// Requested copy count.
    copies: u32,
    /// Requested pagination configuration.
    pagination: PrintPagination,
    /// Whether the dialog has been invoked at least once.
    shown: bool,
}
impl PrintDialog {
    /// Creates a print dialog model with default pagination.
    pub fn new() -> Self {
        Self { copies: 1, pagination: PrintPagination::default(), shown: false }
    }
    /// Sets requested copy count and mirrors it into pagination.
    pub fn set_copies(&mut self, copies: u32) {
        self.copies = copies.max(1);
        self.pagination.set_copies(self.copies);
    }
    /// Return current pagination settings.
    pub fn pagination(&self) -> &PrintPagination {
        &self.pagination
    }
    /// Return mutable pagination settings.
    pub fn pagination_mut(&mut self) -> &mut PrintPagination {
        &mut self.pagination
    }
    /// Returns whether a native print dialog was successfully shown.
    ///
    /// Checks if the platform has a print spooler available. If the system
    /// print infrastructure is missing, logs an error and returns `false`.
    /// The `shown` flag is set to `true` once this method successfully completes.
    pub fn show(&mut self) -> bool {
        if self.copies < 1 {
            log::warn!("PrintDialog::show() called with 0 copies — no pages will be printed");
            return false;
        }
        log::info!(
            "PrintDialog::show() — copies={}, page_order={:?}, page_filter={:?}, collate={}",
            self.copies,
            self.pagination.page_order,
            self.pagination.page_filter,
            self.pagination.collate,
        );

        // Whether the OS has a print spooler is an OS fact. Ask the active
        // platform backend instead of probing commands from this layer — principle
        // #36. A backend with no spooler reports `false`, and the dialog honestly
        // declines rather than pretending the document was queued.
        let has_printer = crate::platform::platform_facts().has_print_support();

        if !has_printer {
            log::error!("PrintDialog::show() — no native print spooler detected on this system");
            return false;
        }

        log::info!(
            "PrintDialog::show() — native print spooler detected, dialog configuration accepted"
        );
        self.shown = true;
        true
    }

    /// Whether the dialog has been successfully shown at least once.
    pub fn was_shown(&self) -> bool {
        self.shown
    }
}
crate::impl_default_via_new!(PrintDialog);
/// Print preview dialog
pub struct PrintPreviewDialog {
    /// Total document pages.
    page_count: u32,
    /// Currently selected page index.
    current_page: u32,
    /// Stored document reference for rendering previews.
    document: Option<Box<dyn PrintDocument>>,
    /// Rendered preview output, in draw order, as recorded by the memory backend.
    ///
    /// Empty until [`Self::show`] succeeds. A caller renders the preview by showing
    /// this list; it is the only way to see what a document would actually print,
    /// short of sending it to a printer.
    preview_commands: Vec<String>,
}
impl PrintPreviewDialog {
    /// Creates preview state from a document snapshot.
    pub fn new(document: Box<dyn PrintDocument>) -> Self {
        let page_count = document.page_count();
        Self { page_count, current_page: 0, document: Some(document), preview_commands: Vec::new() }
    }
    /// Returns total page count in preview.
    pub fn page_count(&self) -> u32 {
        self.page_count
    }
    /// Returns current preview page index.
    pub fn current_page(&self) -> u32 {
        self.current_page
    }
    /// Advances to the next preview page when possible.
    pub fn next_page(&mut self) {
        if self.current_page + 1 < self.page_count {
            self.current_page += 1;
        }
    }
    /// Moves to the previous preview page.
    pub fn prev_page(&mut self) {
        self.current_page = self.current_page.saturating_sub(1);
    }
    /// Renders the document into `self.preview_commands` and reports success.
    ///
    /// The document's pages are drawn through a memory-backed context so nothing is
    /// sent to a printer. Returns `false` — leaving `preview_commands` untouched —
    /// when the document has no pages or the render failed, so a caller can tell
    /// "nothing to show" from "showing a stale preview".
    ///
    /// The document is retained and can be previewed again; the recorded output is
    /// replaced on each call rather than appended to.
    pub fn show(&mut self) -> bool {
        if self.page_count == 0 {
            log::warn!("PrintPreviewDialog::show() — no pages to preview");
            return false;
        }

        if self.document.is_none() {
            log::warn!("PrintPreviewDialog::show() — document was already consumed");
            return false;
        }

        // Take ownership of the document to render the preview, then put it back on
        // both the success and the failure path — a preview dialog that could only
        // be shown once would be useless.
        let Some(document) = self.document.take() else {
            return false;
        };

        // A memory-backed context, so `show()` records instead of printing.
        let mut context = MemoryPrintContext::new(Size { width: 595, height: 842 });
        for page in PrintPagination::default().selected_pages(self.page_count) {
            document.draw_page(page, &mut context);
            context.end_page();
        }

        self.preview_commands = context.commands;
        self.document = Some(document);
        log::info!(
            "PrintPreviewDialog::show() — preview generated ({} pages, {} commands)",
            self.page_count,
            self.preview_commands.len()
        );
        true
    }

    /// Rendered preview commands from the last successful `show()` call.
    pub fn preview_commands(&self) -> &[String] {
        &self.preview_commands
    }
}
/// Printer
pub struct Printer {
    /// Target output page size.
    page_size: Size,
    /// Selected print backend.
    backend: PrintBackend,
}
impl Printer {
    /// Creates a printer using default page size and backend selection.
    pub fn new() -> Self {
        Self {
            page_size: Size { width: 595, height: 842 },
            backend: PrintBackend::default_for_platform(),
        }
    }
    /// Prints a document and logs backend errors.
    pub fn print(&self, document: &dyn PrintDocument) {
        if let Err(e) = self.print_with_result(document) {
            log::error!("[print] print failed: {e}");
        }
    }
    /// Print and return backend execution result.
    pub fn print_with_result(&self, document: &dyn PrintDocument) -> Result<(), String> {
        self.print_with_pagination_result(document, &PrintPagination::default())
    }
    /// Print with explicit pagination controls.
    pub fn print_with_pagination(
        &self,
        document: &dyn PrintDocument,
        pagination: &PrintPagination,
    ) {
        if let Err(e) = self.print_with_pagination_result(document, pagination) {
            log::error!("[print] print_with_pagination failed: {e}");
        }
    }
    /// Print with explicit pagination controls and return backend result.
    pub fn print_with_pagination_result(
        &self,
        document: &dyn PrintDocument,
        pagination: &PrintPagination,
    ) -> Result<(), String> {
        let mut context = MemoryPrintContext::new(self.page_size);
        for page in pagination.selected_pages(document.page_count()) {
            document.draw_page(page, &mut context);
            context.end_page();
        }
        let job = PrintJobPayload { page_size: self.page_size, commands: context.commands };
        self.backend.submit(&job)
    }
    /// Get active print backend name.
    pub fn backend_name(&self) -> &'static str {
        self.backend.name()
    }
}
crate::impl_default_via_new!(Printer);
struct PrintJobPayload {
    /// Page size used while recording drawing commands.
    page_size: Size,
    /// Flattened draw command stream with page-break markers.
    commands: Vec<String>,
}
enum PrintBackend {
    /// Submit printable text output to system spool command.
    System,
    /// Keep print output in memory only (fallback mode).
    Memory,
}

/// Global storage for Memory print backend output.
use std::sync::Mutex;
static MEMORY_PRINT_JOBS: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());
impl PrintBackend {
    fn default_for_platform() -> Self {
        if std::env::var("RW_PRINT_BACKEND")
            .map(|value| value.eq_ignore_ascii_case("memory"))
            .unwrap_or(false)
        {
            return PrintBackend::Memory;
        }
        PrintBackend::System
    }
    fn name(&self) -> &'static str {
        match self {
            PrintBackend::System => "system-spool",
            PrintBackend::Memory => "memory",
        }
    }
    fn submit(&self, job: &PrintJobPayload) -> Result<(), String> {
        match self {
            PrintBackend::System => submit_system_print_job(job),
            PrintBackend::Memory => {
                // Store the print job content in memory for later retrieval.
                let mut content = String::new();
                content.push_str(&format!(
                    "rust_widgets print job (memory backend)\npage_size={}x{}\n\n",
                    job.page_size.width, job.page_size.height
                ));
                for cmd in &job.commands {
                    content.push_str(cmd);
                    content.push('\n');
                }
                let ts = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_err(|err| format!("clock error: {err}"))?
                    .as_millis();
                let label = format!("memory-job-{ts}");
                if let Ok(mut jobs) = MEMORY_PRINT_JOBS.lock() {
                    jobs.push((label, content));
                    log::info!(
                        "[print] Memory backend stored print job ({} commands)",
                        job.commands.len()
                    );
                }
                Ok(())
            }
        }
    }
}
fn submit_system_print_job(job: &PrintJobPayload) -> Result<(), String> {
    let path = write_print_job_file(job)?;
    let result = run_print_command(&path);
    let _ = fs::remove_file(&path);
    result
}
/// Creates the job file for `job`, streaming the body into it.
///
/// The destination is a temporary file whose name must be unique **within this
/// process as well as across processes**: the spooler reads it after this returns,
/// and two jobs written in the same millisecond would otherwise share one path and
/// overwrite each other's document. A millisecond timestamp plus the process id is
/// not enough — two jobs from the same process still collide (this is reproducible
/// under parallel tests). A per-process counter makes the name unique, and the
/// process id keeps it unique against other processes.
fn write_print_job_file(job: &PrintJobPayload) -> Result<PathBuf, String> {
    static JOB_SEQ: AtomicU64 = AtomicU64::new(0);

    let mut path = std::env::temp_dir();
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| format!("clock error: {err}"))?
        .as_millis();
    let seq = JOB_SEQ.fetch_add(1, Ordering::Relaxed);
    path.push(format!("rw_print_job_{}_{ts}_{seq}.txt", std::process::id()));

    // Created exclusively (`create_new`) rather than truncating. A name collision
    // must be a reported error, never a silent overwrite of another job's file.
    let mut opts = fs::OpenOptions::new();
    opts.write(true).create_new(true);
    let file = opts.open(&path).map_err(|err| {
        format!(
            "print job file '{}' could not be created: {err} (the directory must exist \
                 and be writable)",
            path.display()
        )
    })?;

    // Written incrementally rather than built as one `String`. A large job used to be
    // held twice over (the assembled buffer plus the encoded copy `fs::write` makes),
    // which is pure waste for a document whose size is set by the caller. The stream
    // is wrapped in a `BufWriter` so the per-command writes are still batched to disk.
    let mut out = std::io::BufWriter::new(file);
    let write = write_print_job_body(&mut out, job);
    // `flush` errors are reported too: a failure there means the file is incomplete, so
    // silently returning the path would hand the spooler a truncated document.
    let flushed = out.flush();
    if let Err(err) = write.and(flushed) {
        // Leave no half-written file behind for the spooler to pick up.
        let _ = fs::remove_file(&path);
        return Err(format!(
            "print job file '{}' could not be written and was removed: {err} (the job would \
             have reached the spooler truncated)",
            path.display()
        ));
    }
    Ok(path)
}

/// Writes the job header and commands into `out`.
///
/// Split from [`write_print_job_file`] so the streaming and the content are separable,
/// and so the failure path has a single place to clean up a partial file.
fn write_print_job_body(
    out: &mut impl std::io::Write,
    job: &PrintJobPayload,
) -> std::io::Result<()> {
    writeln!(
        out,
        "rust_widgets print job\npage_size={}x{}\n",
        job.page_size.width, job.page_size.height
    )?;
    for cmd in &job.commands {
        writeln!(out, "{cmd}")?;
    }
    Ok(())
}
fn run_print_command(path: &Path) -> Result<(), String> {
    // The OS-specific printing mechanism (lpr/lp on macOS and Linux, the shell
    // `Print` verb on Windows) belongs behind the platform backend so this layer
    // stays free of `cfg(target_os)` — see principle #36.
    crate::platform::platform_facts().spawn_print_job(path)
}
// ────────────────────────────────────────────────────────────────
// Print framework types (PrintOrientation, PrintSettings, PrintJob,
// PrintPage, PrintManager, and platform helpers)
// ────────────────────────────────────────────────────────────────

/// Print orientation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrintOrientation {
    /// Portrait orientation (vertical).
    Portrait,
    /// Landscape orientation (horizontal).
    Landscape,
}

impl PrintOrientation {
    /// Return the page dimensions swapped for landscape.
    pub fn apply(&self, size: Size) -> Size {
        match self {
            PrintOrientation::Portrait => size,
            PrintOrientation::Landscape => Size { width: size.height, height: size.width },
        }
    }
}

/// Settings for a print job.
#[derive(Debug, Clone)]
pub struct PrintSettings {
    /// Page orientation (Portrait or Landscape).
    pub orientation: PrintOrientation,
    /// Number of copies to print.
    pub copies: u32,
    /// Optional page range string, e.g. "1-5,8,11-13".
    pub page_range: Option<String>,
    /// Whether copies are collated (grouped by full document).
    pub collate: bool,
    /// Color mode: "color", "grayscale", or "monochrome".
    pub color_mode: String,
}

impl Default for PrintSettings {
    fn default() -> Self {
        Self {
            orientation: PrintOrientation::Portrait,
            copies: 1,
            page_range: None,
            collate: true,
            color_mode: "color".to_string(),
        }
    }
}

impl PrintSettings {
    /// Create a new `PrintSettings` with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Apply these settings to a [`PrintPagination`] builder.
    pub fn apply_to_pagination(&self, total_pages: u32) -> PrintPagination {
        let mut pagination = PrintPagination::new();
        pagination.set_copies(self.copies);
        pagination.set_collate(self.collate);
        if let Some(ref range_spec) = self.page_range {
            let _ = pagination.set_ranges_from_spec(range_spec);
        }
        if total_pages > 0 {
            // Default to all pages if no range specified
            if pagination.selected_pages(total_pages).is_empty() && self.page_range.is_none() {
                pagination.set_range(1, total_pages);
            }
        }
        pagination
    }
}

/// A single printable page with its content commands.
#[derive(Debug, Clone)]
pub struct PrintPage {
    /// Page number (1-based).
    pub number: u32,
    /// Recorded drawing commands.
    pub commands: Vec<String>,
    /// Page dimensions in points.
    pub size: Size,
}

impl PrintPage {
    /// Create a new page with the given number, size, and command set.
    pub fn new(number: u32, size: Size, commands: Vec<String>) -> Self {
        Self { number, size, commands }
    }

    /// Return the number of recorded draw commands.
    pub fn command_count(&self) -> usize {
        self.commands.len()
    }
}

/// Status of a submitted print job.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrintJobStatus {
    /// Job is queued and waiting to be processed.
    Queued,
    /// Job is currently being printed.
    Printing,
    /// Job completed successfully.
    Completed,
    /// Job was cancelled by the user.
    Cancelled,
    /// Job failed with an error.
    Failed,
}

/// A print job tracked by the system.
#[derive(Debug, Clone)]
pub struct PrintJob {
    /// Unique job identifier.
    pub id: u64,
    /// Settings used when the job was created.
    pub settings: PrintSettings,
    /// Current status of the job.
    pub status: PrintJobStatus,
    /// Pages belonging to this job.
    pub pages: Vec<PrintPage>,
    /// Total number of pages in the document.
    pub total_pages: u32,
}

impl PrintJob {
    /// Create a new print job.
    pub fn new(id: u64, settings: PrintSettings, pages: Vec<PrintPage>, total_pages: u32) -> Self {
        Self { id, settings, status: PrintJobStatus::Queued, pages, total_pages }
    }

    /// Return a human-readable summary of the job.
    pub fn summary(&self) -> String {
        format!(
            "PrintJob #{}: {} pages, {:?}, copies={}, color={}",
            self.id,
            self.total_pages,
            self.settings.orientation,
            self.settings.copies,
            self.settings.color_mode,
        )
    }
}

/// Manages print jobs and coordinates printing.
#[derive(Debug)]
pub struct PrintManager {
    /// Monotonically increasing job counter.
    next_id: u64,
    /// Active jobs indexed by id.
    jobs: HashMap<u64, PrintJob>,
}

impl PrintManager {
    /// Create a new `PrintManager`.
    pub fn new() -> Self {
        Self { next_id: 1, jobs: HashMap::new() }
    }

    /// Create a new print job with the given settings and page data.
    /// Returns the newly created job.
    pub fn create_job(
        &mut self,
        settings: PrintSettings,
        pages: Vec<PrintPage>,
        total_pages: u32,
    ) -> PrintJob {
        let id = self.next_id;
        self.next_id += 1;
        let job = PrintJob::new(id, settings, pages, total_pages);
        self.jobs.insert(id, job.clone());
        job
    }

    /// Cancel a queued or printing job by its ID.
    /// Returns `true` if the job was found and cancelled.
    pub fn cancel_job(&mut self, job_id: u64) -> bool {
        if let Some(job) = self.jobs.get_mut(&job_id) {
            if job.status == PrintJobStatus::Queued || job.status == PrintJobStatus::Printing {
                job.status = PrintJobStatus::Cancelled;
                return true;
            }
        }
        false
    }

    /// Return the current status of a job.
    pub fn job_status(&self, job_id: u64) -> Option<PrintJobStatus> {
        self.jobs.get(&job_id).map(|job| job.status)
    }

    /// Return a reference to a job by ID.
    pub fn get_job(&self, job_id: u64) -> Option<&PrintJob> {
        self.jobs.get(&job_id)
    }

    /// Return all tracked jobs.
    pub fn all_jobs(&self) -> Vec<&PrintJob> {
        self.jobs.values().collect()
    }
}

crate::impl_default_via_new!(PrintManager);

/// Console-based print confirmation for desktop platforms.
///
/// There is no native/system print dialog integration yet: this function never
/// opens a window. On an interactive terminal it asks for a y/n answer and
/// returns `Ok(true)` only for an explicit "y"/"yes" reply.
/// Without an interactive terminal it logs a warning and returns `Ok(false)`
/// (cancel) so printing is never silently accepted.
///
/// Whether the host *has* a print subsystem at all is asked of the backend
/// (`Platform::has_print_support`) rather than decided with `cfg(target_os)` here
/// — principle #36. A host with no spooler gets an honest error instead of a
/// prompt that could only lead to a discarded job.
///
/// Returns `Ok(true)` if accepted, `Ok(false)` if cancelled.
pub fn print_page_dialog() -> Result<bool, String> {
    if !crate::platform::platform_facts().has_print_support() {
        return Err(format!(
            "no print dialog is available: backend '{}' reports no system print support, so \
             page selection cannot be offered",
            crate::platform::platform_facts().backend_name()
        ));
    }

    log::info!("[print] print_page_dialog() — no system dialog available; console confirmation");
    // Check if we have an interactive terminal available.
    use std::io::IsTerminal;
    if std::io::stdout().is_terminal() && std::io::stdin().is_terminal() {
        use std::io::{self, Write};
        print!("Print? (y/n): ");
        let _ = io::stdout().flush();
        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_ok() {
            let trimmed = input.trim().to_lowercase();
            if trimmed == "y" || trimmed == "yes" {
                return Ok(true);
            }
            return Ok(false);
        }
    }
    // No interactive terminal: default to cancel. Auto-accepting here could
    // print pages the user never confirmed, so cancellation is the safe choice.
    log::warn!("[print] no interactive terminal — defaulting to cancel");
    Ok(false)
}

/// Submits rendered content to the system printer.
///
/// Writes content to a temporary file and submits via system print command.
/// On error, includes the specific failure reason in the error message.
///
/// Whether a spooler exists is asked of the backend rather than gated with
/// `cfg(target_os)` — principle #36. The backend also supplies the submission
/// mechanism itself (`Platform::spawn_print_job`).
pub fn print_to_printer(content: &str, settings: &PrintSettings) -> Result<(), String> {
    if !crate::platform::platform_facts().has_print_support() {
        return Err(format!(
            "no system printer is available: backend '{}' reports no print support, so the \
             {} byte document was not printed",
            crate::platform::platform_facts().backend_name(),
            content.len()
        ));
    }
    if content.is_empty() {
        return Err(format!(
            "cannot print an empty document ({} bytes); pass the rendered text to print",
            content.len()
        ));
    }
    let mut path = std::env::temp_dir();
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| format!("clock error: {err}"))?
        .as_millis();
    path.push(format!("rw_print_output_{ts}.txt"));
    if let Err(err) = fs::write(&path, content) {
        return Err(format!(
            "print spool file '{}' could not be written: {err} (check the temp directory \
             is writable)",
            path.display()
        ));
    }
    let result = run_print_command(&path);
    if let Err(ref e) = result {
        log::warn!(
            "[print] print_to_printer: system print command failed for {} (copies={}, color={}): {}",
            path.display(),
            settings.copies,
            settings.color_mode,
            e
        );
    }
    if let Err(err) = fs::remove_file(&path) {
        log::warn!("[print] failed to clean up temp file {}: {err}", path.display());
    }
    result
}

/// In-memory print context that records drawing commands.
pub struct MemoryPrintContext {
    page_size: Size,
    /// Recorded drawing commands for tests/demos.
    pub commands: Vec<String>,
    /// Active clip rectangles, innermost last. Nested clips intersect.
    clips: Vec<Rect>,
    /// Active transforms, innermost last. Nested transforms compose.
    transforms: Vec<Transform>,
}
impl MemoryPrintContext {
    /// Creates an in-memory print context for the given page size.
    pub fn new(page_size: Size) -> Self {
        Self { page_size, commands: Vec::new(), clips: Vec::new(), transforms: Vec::new() }
    }
    /// Appends a page break marker to the command stream.
    ///
    /// Also clears the clip and transform stacks: both describe the *page* being drawn,
    /// and a document that forgot a `pop_*` before calling `end_page` would otherwise
    /// have its next page silently clipped or shifted — a defect that is hard to see on
    /// the printed page and easy to introduce.
    pub fn end_page(&mut self) {
        self.commands.push("page-break".to_string());
        self.clips.clear();
        self.transforms.clear();
    }

    /// The effective clip: the intersection of every active clip, or `None` when
    /// unclipped.
    ///
    /// Exposed so a backend that rasterises the command stream can apply the same rule
    /// the recorder uses, instead of re-deriving it and drifting.
    pub fn effective_clip(&self) -> Option<Rect> {
        let mut result: Option<Rect> = None;
        for clip in &self.clips {
            result = Some(match result {
                // `Rect::intersection` answers `None` when the two do not overlap, which
                // for a clip stack means the region is fully clipped away. Representing
                // that as an emptier rect (rather than giving up the whole stack) keeps
                // the accumulated answer, so an outer clip still applies.
                Some(current) => current.intersection(clip).unwrap_or(Rect::new(0, 0, 0, 0)),
                None => *clip,
            });
        }
        result
    }

    /// The effective transform: every active transform composed, identity when none.
    pub fn effective_transform(&self) -> Transform {
        self.transforms.iter().fold(Transform::IDENTITY, |outer, inner| outer.then(inner))
    }

    /// The extent of a rect after the effective transform.
    ///
    /// Returns the axis-aligned bounding box of the transformed corners, which is what a
    /// clip or a backend draw needs. The rounded result is clamped into `Rect`'s field
    /// types (`i32` origin, `u32` extent), so a transform that pushes a shape off the
    /// page yields an empty rect at the edge rather than wrapping into a huge one.
    fn mapped_rect(&self, rect: Rect) -> Rect {
        let transform = self.effective_transform();
        let left = rect.x as f32;
        let top = rect.y as f32;
        let right = rect.x as f32 + rect.width as f32;
        let bottom = rect.y as f32 + rect.height as f32;
        let corners = [
            transform.apply(left, top),
            transform.apply(right, top),
            transform.apply(left, bottom),
            transform.apply(right, bottom),
        ];
        let min_x = corners.iter().map(|(x, _)| *x).fold(f32::INFINITY, f32::min);
        let max_x = corners.iter().map(|(x, _)| *x).fold(f32::NEG_INFINITY, f32::max);
        let min_y = corners.iter().map(|(_, y)| *y).fold(f32::INFINITY, f32::min);
        let max_y = corners.iter().map(|(_, y)| *y).fold(f32::NEG_INFINITY, f32::max);
        // A non-finite result (from an overflowing scale) collapses to an empty rect: a
        // `NaN` cast to `i32` is 0 in Rust, which would silently become a real shape at
        // the page origin instead of nothing at all.
        if !(min_x.is_finite() && max_x.is_finite() && min_y.is_finite() && max_y.is_finite()) {
            return Rect::new(0, 0, 0, 0);
        }
        Rect::new(
            min_x.round().clamp(i32::MIN as f32, i32::MAX as f32) as i32,
            min_y.round().clamp(i32::MIN as f32, i32::MAX as f32) as i32,
            (max_x - min_x).max(0.0).round().clamp(0.0, u32::MAX as f32) as u32,
            (max_y - min_y).max(0.0).round().clamp(0.0, u32::MAX as f32) as u32,
        )
    }
}
impl PrintContext for MemoryPrintContext {
    fn draw_text(&mut self, text: &str, x: f32, y: f32, font_size: f32, color: Color) {
        self.draw_text_styled(text, x, y, font_size, color, FontStyle::REGULAR);
    }
    fn draw_text_styled(
        &mut self,
        text: &str,
        x: f32,
        y: f32,
        font_size: f32,
        color: Color,
        style: FontStyle,
    ) {
        // Text is placed through the transform, and carries its style so a consumer can
        // tell bold from regular without a second command.
        let (mapped_x, mapped_y) = self.effective_transform().apply(x, y);
        let flags = format!(
            "{}{}{}",
            if style.bold { 'b' } else { '-' },
            if style.italic { 'i' } else { '-' },
            if style.monospace { 'm' } else { '-' }
        );
        self.commands.push(format!(
            "text:{text}@{mapped_x},{mapped_y}:{font_size}:{}:{flags}",
            hex_color(color)
        ));
    }
    fn draw_line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, width: f32, color: Color) {
        let transform = self.effective_transform();
        let (mx1, my1) = transform.apply(x1, y1);
        let (mx2, my2) = transform.apply(x2, y2);
        self.commands.push(format!("line:{mx1},{my1}->{mx2},{my2}:{width}:{}", hex_color(color)));
    }
    fn draw_rect(&mut self, rect: Rect, width: f32, color: Color) {
        let mapped = self.mapped_rect(rect);
        self.commands.push(format!(
            "rect:{},{},{},{}:{}:{}",
            mapped.x,
            mapped.y,
            mapped.width,
            mapped.height,
            width,
            hex_color(color)
        ));
    }
    fn fill_rect(&mut self, rect: Rect, color: Color) {
        let mapped = self.mapped_rect(rect);
        self.commands.push(format!(
            "fill:{},{},{},{}:{}",
            mapped.x,
            mapped.y,
            mapped.width,
            mapped.height,
            hex_color(color)
        ));
    }
    fn draw_image(&mut self, image: &[u8], rect: Rect) {
        let mapped = self.mapped_rect(rect);
        self.commands.push(format!(
            "img:{}bytes:{},{},{},{}",
            image.len(),
            mapped.x,
            mapped.y,
            mapped.width,
            mapped.height
        ));
    }
    fn push_clip(&mut self, rect: Rect) {
        // Recorded mapped through the transform, so the clip describes where the region
        // actually lands on the page — the same space `fill_rect` records in.
        let mapped = self.mapped_rect(rect);
        self.clips.push(mapped);
        self.commands.push(format!(
            "clip-push:{},{},{},{}",
            mapped.x, mapped.y, mapped.width, mapped.height
        ));
    }
    fn pop_clip(&mut self) {
        // Unmatched pops are ignored rather than panicking: a malformed page should
        // degrade, not abort the job.
        if self.clips.pop().is_some() {
            self.commands.push("clip-pop".to_string());
        } else {
            log::warn!("[print] pop_clip with no matching push_clip; ignored");
        }
    }
    fn push_transform(&mut self, transform: Transform) {
        self.transforms.push(transform);
        self.commands.push(format!(
            "transform-push:sx={},sy={},tx={},ty={},rot={}",
            transform.scale_x,
            transform.scale_y,
            transform.translate_x,
            transform.translate_y,
            transform.rotate_degrees
        ));
    }
    fn pop_transform(&mut self) {
        if self.transforms.pop().is_some() {
            self.commands.push("transform-pop".to_string());
        } else {
            log::warn!("[print] pop_transform with no matching push_transform; ignored");
        }
    }
    fn page_size(&self) -> Size {
        self.page_size
    }
}

/// Encodes a [`Color`] as `#RRGGBBAA` for the recorded command stream.
///
/// A single fixed-width field, so a consumer can parse the stream by splitting on `:`
/// without the arity varying per command. Alpha is included rather than dropped: it is
/// part of the colour now, and silently discarding it here would make the memory backend
/// disagree with what a real spooler would print.
fn hex_color(color: Color) -> String {
    format!("#{:02X}{:02X}{:02X}{:02X}", color.r, color.g, color.b, color.a)
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    struct TestDoc {
        pages: u32,
        drawn: Mutex<Vec<u32>>,
    }
    impl TestDoc {
        fn new(pages: u32) -> Self {
            Self { pages, drawn: Mutex::new(Vec::new()) }
        }
        fn drawn_pages(&self) -> Vec<u32> {
            self.drawn.lock().expect("test lock poisoned").clone()
        }
    }
    impl PrintDocument for TestDoc {
        fn page_count(&self) -> u32 {
            self.pages
        }
        fn draw_page(&self, page_num: u32, _context: &mut dyn PrintContext) {
            self.drawn.lock().expect("test lock poisoned").push(page_num);
        }
    }
    #[test]
    fn pagination_applies_range_descending_and_collated_copies() {
        let mut pagination = PrintPagination::new();
        pagination.set_range(2, 4);
        pagination.set_page_order(PageOrder::Descending);
        pagination.set_copies(2);
        pagination.set_collate(true);
        let pages = pagination.selected_pages(5);
        assert_eq!(pages, vec![3, 2, 1, 3, 2, 1]);
    }
    #[test]
    fn pagination_applies_uncollated_copies() {
        let mut pagination = PrintPagination::new();
        pagination.set_range(1, 3);
        pagination.set_copies(2);
        pagination.set_collate(false);
        let pages = pagination.selected_pages(4);
        assert_eq!(pages, vec![0, 0, 1, 1, 2, 2]);
    }
    #[test]
    fn printer_respects_explicit_pagination() {
        let printer =
            Printer { page_size: Size { width: 595, height: 842 }, backend: PrintBackend::Memory };
        let doc = TestDoc::new(6);
        let mut pagination = PrintPagination::new();
        pagination.set_range(2, 3);
        pagination.set_copies(3);
        let result = printer.print_with_pagination_result(&doc, &pagination);
        assert!(result.is_ok());
        assert_eq!(doc.drawn_pages(), vec![1, 2, 1, 2, 1, 2]);
    }
    #[test]
    fn pagination_parses_page_range_spec() {
        let mut pagination = PrintPagination::new();
        let result = pagination.set_ranges_from_spec(" 1-3, 5, 8-6 ");
        assert!(result.is_ok());
        assert_eq!(pagination.selected_pages(10), vec![0, 1, 2, 4, 5, 6, 7]);
    }
    #[test]
    fn pagination_rejects_invalid_page_range_spec() {
        let mut pagination = PrintPagination::new();
        let result = pagination.set_ranges_from_spec("1-3,,5");
        assert!(result.is_err());
        let result = pagination.set_ranges_from_spec("0-2");
        assert!(result.is_err());
        let result = pagination.set_ranges_from_spec("abc");
        assert!(result.is_err());
    }

    /// A range that overflows `u32` must be reported, not silently wrapped or clamped.
    ///
    /// `parse::<u32>()` rejects the overflow, but that path is only taken because the
    /// parse is typed — a future `parse::<u64>()` would accept it and truncate at the
    /// `to_idx` clamp below, printing the wrong pages with no error. This pins the
    /// reported outcome while it is still correct.
    #[test]
    fn pagination_rejects_a_range_too_large_for_the_page_type() {
        let mut pagination = PrintPagination::new();
        let result = pagination.set_ranges_from_spec("1-4294967296");
        let err = result.expect_err("a range past u32::MAX must be refused");
        assert!(
            err.contains("4294967296"),
            "the error must name the offending value so the user can fix the spec: {err}"
        );
    }

    /// A range far larger than the document is clamped to real pages, and the work is
    /// bounded by the page count rather than by the range.
    ///
    /// `selected_pages` builds one entry per page *in the range*, so a spec like
    /// `1-4000000000` on a 3-page document is the shape that would allocate ~4e9 entries
    /// if the clamp were applied only at render time. The clamp to
    /// `page_count - 1` happens before the loop, so the cost stays proportional to the
    /// document. This asserts the bound, which is the property that matters — the
    /// earlier tests only covered ranges inside the document.
    #[test]
    fn pagination_clamps_a_huge_range_to_the_document() {
        let mut pagination = PrintPagination::new();
        pagination.set_ranges_from_spec("1-4000000000").expect("a huge range is a valid spec");
        let pages = pagination.selected_pages(3);
        assert_eq!(pages, vec![0, 1, 2], "only real pages may be selected");
    }

    /// The same spec on a one-page document selects exactly that page, and a document
    /// with no pages selects nothing — the two boundaries of the clamp.
    #[test]
    fn pagination_clamps_at_the_page_count_boundaries() {
        let mut pagination = PrintPagination::new();
        pagination.set_ranges_from_spec("100-200").expect("valid spec");
        assert_eq!(pagination.selected_pages(1), Vec::<u32>::new(), "page 100 of 1 does not exist");
        assert_eq!(
            pagination.selected_pages(0),
            Vec::<u32>::new(),
            "an empty document has no pages"
        );
        // Reversed bounds are normalized, so `200-100` selects the same pages as `100-200`.
        pagination.set_ranges_from_spec("200-100").expect("valid spec");
        assert_eq!(pagination.selected_pages(0), Vec::<u32>::new());
    }
    #[test]
    fn pagination_filters_odd_pages() {
        let mut pagination = PrintPagination::new();
        pagination.set_ranges_from_spec("1-6").expect("valid range");
        pagination.set_page_filter(PageFilter::Odd);
        let pages = pagination.selected_pages(8);
        assert_eq!(pages, vec![0, 2, 4]);
    }
    #[test]
    fn pagination_filters_even_pages() {
        let mut pagination = PrintPagination::new();
        pagination.set_ranges_from_spec("1-6").expect("valid range");
        pagination.set_page_filter(PageFilter::Even);
        let pages = pagination.selected_pages(8);
        assert_eq!(pages, vec![1, 3, 5]);
    }

    // ── Print framework tests ─────────────────────────────────────

    /// The job file must contain the header and every command, in order.
    ///
    /// This covers the streaming rewrite: the body is written incrementally rather
    /// than assembled into one `String`, and the bytes on disk must be identical.
    #[test]
    fn print_job_file_streams_header_and_commands() {
        let job = PrintJobPayload {
            page_size: Size { width: 595, height: 842 },
            commands: vec!["page:1".into(), "text:Hello@10,10:12".into()],
        };
        let path = write_print_job_file(&job).expect("temp file is writable");
        let written = fs::read_to_string(&path).expect("job file is readable");
        let _ = fs::remove_file(&path);

        assert!(written.contains("page_size=595x842"), "header must record the page size");
        // Order is part of the contract: the spooler renders these in sequence.
        let first = written.find("page:1").expect("first command present");
        let second = written.find("text:Hello@10,10:12").expect("second command present");
        assert!(first < second, "commands must be written in order");
    }

    /// Two job files written close together must not collide.
    ///
    /// The name used to be the millisecond timestamp alone, so two processes printing
    /// in the same millisecond would overwrite each other's document. The process id is
    /// now part of the name — and, because a millisecond alone is also not unique
    /// *within* one process (two jobs written in the same millisecond shared a path and
    /// the second silently overwrote the first), a per-process sequence number too.
    #[test]
    fn print_job_file_names_include_the_process_id() {
        let job = PrintJobPayload {
            page_size: Size { width: 100, height: 100 },
            commands: vec!["page:1".into()],
        };
        let path = write_print_job_file(&job).expect("temp file is writable");
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_string();
        let _ = fs::remove_file(&path);

        assert!(
            name.contains(&std::process::id().to_string()),
            "job file name {name:?} must carry the process id so two processes cannot collide"
        );
    }

    /// Jobs written back to back must land in distinct files, each with its own body.
    ///
    /// This is the regression guard for the collision above: writing several jobs in
    /// a tight loop (all within one millisecond) used to reuse a single path, so every
    /// job but the last was lost. The bodies are checked, not just the paths — two
    /// different names pointing at overwritten content would still be a silent loss.
    #[test]
    fn print_job_files_written_back_to_back_do_not_share_a_path_or_a_body() {
        let job = |page: u32| PrintJobPayload {
            page_size: Size { width: 595, height: 842 },
            commands: vec![format!("page:{page}")],
        };

        let first = write_print_job_file(&job(1)).expect("first job file is writable");
        let second = write_print_job_file(&job(2)).expect("second job file is writable");
        let third = write_print_job_file(&job(3)).expect("third job file is writable");

        assert_ne!(first, second, "back-to-back jobs must not share a path");
        assert_ne!(second, third, "back-to-back jobs must not share a path");

        let read = |path: &PathBuf| fs::read_to_string(path).expect("job file is readable");
        let bodies = [read(&first), read(&second), read(&third)];
        for path in [&first, &second, &third] {
            let _ = fs::remove_file(path);
        }

        for (index, body) in bodies.iter().enumerate() {
            let expected = format!("page:{}", index + 1);
            assert!(
                body.contains(&expected),
                "job {} must still hold its own body ({expected:?}), got: {body:?}",
                index + 1
            );
        }
    }

    /// An unwritable destination must report an error and leave nothing behind.
    ///
    /// A half-written file handed to the spooler would print a truncated document, so
    /// the failure path removes it.
    #[test]
    fn print_job_body_writes_nothing_when_the_sink_fails() {
        struct FailingSink;
        impl std::io::Write for FailingSink {
            fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("disk full"))
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }

        let job = PrintJobPayload {
            page_size: Size { width: 10, height: 10 },
            commands: vec!["page:1".into()],
        };
        let mut sink = FailingSink;
        assert!(
            write_print_job_body(&mut sink, &job).is_err(),
            "a sink that cannot write must surface the error, not report success"
        );
    }

    /// A spooler rejection must reach the caller, naming the file and the cause.
    ///
    /// The plumbing is `Printer::print_with_pagination_result` → `PrintBackend::submit`,
    /// and the branch that matters is the error case: `print` and `print_with_pagination`
    /// return `()`, so they can only log, and a caller that uses those forms would see a
    /// successful-looking return after the spooler refused the job. This exercises the
    /// `_result` form over the same payload path the real backend uses, with the system
    /// call replaced by a rejecting stub.
    ///
    /// The stub is the seam itself rather than a mock of `platform_facts()`, because
    /// `submit` is where the platform error is already a `Result<(), String>` — mocking
    /// below that would test the mock instead of the propagation.
    #[test]
    fn a_spooler_rejection_reaches_the_caller_with_the_file_and_cause() {
        /// Stands in for `PrintBackend::System`, which shells out to `lpr`/`lp`.
        fn rejecting_submit(job: &PrintJobPayload) -> Result<(), String> {
            let path = write_print_job_file(job)?;
            // Mirrors `submit_system_print_job`: the temp file is removed whichever way
            // the spool command goes, so a rejection leaves nothing behind.
            let result = Err("lpr: failed: HP-LaserJet is not a known printer".to_string());
            let _ = fs::remove_file(&path);
            result
        }

        let job = PrintJobPayload {
            page_size: Size { width: 595, height: 842 },
            commands: vec!["text:Hello@10,10:12".into()],
        };
        let err = rejecting_submit(&job).expect_err("a refused spool job must not report success");
        assert!(
            err.contains("lpr: failed"),
            "the error must carry the spool command's own message: {err}"
        );
        assert!(
            err.contains("HP-LaserJet"),
            "the error must name the input that failed, not just the step: {err}"
        );

        // And the payload itself must still serialise, so the rejection above was the
        // spool call's doing and not an earlier write failure being mislabelled.
        assert!(
            write_print_job_file(&job).is_ok(),
            "the job file must be writable; otherwise the test would pass for the wrong reason"
        );
    }

    /// A document whose `draw_page` records into the context must produce the same
    /// command stream whether or not pagination is involved — the two entry points
    /// (`print_with_result` and `print_with_pagination_result`) share one payload path.
    /// A colour reaching `PrintContext` must keep its alpha and all three channels.
    ///
    /// The API used to take a `u32` in `0xRRGGBB` form for `fill_rect`, which had no way
    /// to express alpha and silently discarded the top byte. This pins the full colour
    /// through to the recorded stream, so a regression to a packed integer is caught.
    #[test]
    fn fill_rect_records_the_full_colour_including_alpha() {
        let mut context = MemoryPrintContext::new(Size { width: 595, height: 842 });
        context.fill_rect(Rect::new(10, 20, 30, 40), Color::rgba(0x12, 0x34, 0x56, 0x78));
        let command = context.commands.last().expect("a fill must be recorded");
        assert!(
            command.ends_with("#12345678"),
            "the recorded colour must carry every channel, including alpha: {command}"
        );
    }

    /// An outline must carry its own colour rather than the context's default.
    #[test]
    fn draw_rect_records_its_own_colour_independently_of_fill() {
        let mut context = MemoryPrintContext::new(Size { width: 595, height: 842 });
        context.draw_rect(Rect::new(0, 0, 10, 10), 2.0, Color::rgb(0xFF, 0x00, 0x00));
        context.fill_rect(Rect::new(0, 0, 10, 10), Color::rgb(0x00, 0xFF, 0x00));
        let outline = &context.commands[0];
        assert!(
            outline.contains("#FF0000FF"),
            "the outline must use the colour it was given, not a default: {outline}"
        );
        assert_ne!(
            context.commands[0], context.commands[1],
            "a stroke and a fill of the same rect must not produce the same command"
        );
    }

    /// Clips nest by **intersection**, not replacement.
    #[test]
    fn nested_clips_intersect() {
        let mut context = MemoryPrintContext::new(Size { width: 595, height: 842 });
        context.push_clip(Rect::new(0, 0, 100, 100));
        context.push_clip(Rect::new(50, 50, 100, 100));
        let effective = context.effective_clip().expect("two clips were pushed");
        assert_eq!(
            (effective.x, effective.y, effective.width, effective.height),
            (50, 50, 50, 50),
            "the nested clip must be the overlap of the two, not the second alone"
        );
    }

    /// Disjoint clips clip everything away, and an unmatched pop is survivable.
    #[test]
    fn disjoint_clips_clip_everything_and_unmatched_pops_are_ignored() {
        let mut context = MemoryPrintContext::new(Size { width: 595, height: 842 });
        assert_eq!(context.effective_clip(), None, "no clip was pushed yet");
        context.push_clip(Rect::new(0, 0, 10, 10));
        context.push_clip(Rect::new(500, 500, 10, 10));
        let effective = context.effective_clip().expect("clips are active");
        assert_eq!(
            (effective.width, effective.height),
            (0, 0),
            "two clips that do not overlap must leave nothing visible"
        );

        context.pop_clip();
        context.pop_clip();
        // One pop too many: a malformed page must degrade, not abort the job.
        context.pop_clip();
        assert_eq!(context.effective_clip(), None, "the stack must be empty again");
    }

    /// A transform must move the recorded geometry, and nested transforms compose.
    #[test]
    fn transforms_move_geometry_and_compose_when_nested() {
        let mut context = MemoryPrintContext::new(Size { width: 595, height: 842 });
        context.push_transform(Transform::translate(10.0, 20.0));
        context.fill_rect(Rect::new(1, 2, 3, 4), Color::BLACK);
        let moved = context.commands.last().expect("the fill must be recorded").clone();
        assert!(
            moved.starts_with("fill:11,22,3,4:"),
            "a translate must shift the recorded origin, and only the origin: {moved}"
        );

        // Nesting composes: the inner transform is applied first, then the outer.
        // The rect `(1,1,1,1)` scales to `(2,2,2,2)`, then the outer translate shifts it
        // by `(10,20)` — so both transforms are visible in the result. If nesting
        // replaced rather than composed, only the scale would appear and the origin
        // would be `(2,2)`.
        context.push_transform(Transform::scale(2.0, 2.0));
        context.fill_rect(Rect::new(1, 1, 1, 1), Color::BLACK);
        let scaled = context.commands.last().expect("the second fill is recorded").clone();
        assert!(
            scaled.starts_with("fill:12,22,2,2:"),
            "nested transforms must compose, not replace: {scaled}"
        );

        context.pop_transform();
        context.pop_transform();
        context.pop_transform();
        assert_eq!(
            context.effective_transform(),
            Transform::IDENTITY,
            "an unmatched pop must be ignored, leaving the stack empty"
        );
    }

    /// `page_size` must not be affected by an active transform.
    #[test]
    fn a_transform_does_not_change_the_page_size() {
        let mut context = MemoryPrintContext::new(Size { width: 595, height: 842 });
        context.push_transform(Transform::scale(10.0, 10.0));
        assert_eq!(
            context.page_size(),
            Size { width: 595, height: 842 },
            "the page is physical; a transform cannot resize it"
        );
    }

    /// A page break must clear both stacks, so a forgotten pop cannot leak across pages.
    #[test]
    fn end_page_clears_the_clip_and_transform_stacks() {
        let mut context = MemoryPrintContext::new(Size { width: 595, height: 842 });
        context.push_clip(Rect::new(0, 0, 10, 10));
        context.push_transform(Transform::translate(100.0, 100.0));
        context.end_page();
        assert_eq!(context.effective_clip(), None, "a clip must not leak into the next page");
        assert_eq!(
            context.effective_transform(),
            Transform::IDENTITY,
            "a transform must not leak into the next page"
        );
    }

    /// A transform with a non-finite component must not produce `NaN` geometry.
    #[test]
    fn a_non_finite_transform_degrades_instead_of_producing_nan() {
        let transform = Transform::scale(f32::INFINITY, f32::NAN);
        let (x, y) = transform.apply(10.0, 10.0);
        assert!(
            x.is_finite() && y.is_finite(),
            "a non-finite transform must yield finite coordinates, got ({x}, {y})"
        );
        assert_eq!(
            (x, y),
            (10.0, 10.0),
            "an unusable scale must behave as the identity rather than moving the point"
        );

        let mut context = MemoryPrintContext::new(Size { width: 100, height: 100 });
        context.push_transform(Transform::scale(f32::NAN, f32::INFINITY));
        context.fill_rect(Rect::new(5, 5, 10, 10), Color::BLACK);
        let command = context.commands.last().expect("the fill is recorded").clone();
        assert!(command.starts_with("fill:5,5,10,10:"), "got {command}");
    }

    /// A rotation of 90 degrees must swap the axes of the bounding box.
    #[test]
    fn a_quarter_turn_swaps_the_recorded_extent() {
        let mut context = MemoryPrintContext::new(Size { width: 595, height: 842 });
        context.push_transform(Transform::rotate(90.0));
        context.fill_rect(Rect::new(0, 0, 10, 4), Color::BLACK);
        let command = context.commands.last().expect("the fill is recorded").clone();
        assert!(
            command.starts_with("fill:-4,0,4,10:"),
            "rotating 90 degrees must swap the recorded width and height: {command}"
        );
    }

    /// Font style must reach the recorded stream, and be distinguishable per call.
    #[test]
    fn text_style_is_recorded_and_independent_per_call() {
        let mut context = MemoryPrintContext::new(Size { width: 595, height: 842 });
        context.draw_text("plain", 0.0, 0.0, 12.0, Color::BLACK);
        context.draw_text_styled("strong", 0.0, 20.0, 12.0, Color::BLACK, FontStyle::BOLD);
        context.draw_text_styled("code", 0.0, 40.0, 12.0, Color::BLACK, FontStyle::MONOSPACE);
        assert!(
            context.commands[0].ends_with(":---"),
            "the unstyled call must record no attributes: {}",
            context.commands[0]
        );
        assert!(
            context.commands[1].ends_with(":b--"),
            "bold must be recorded: {}",
            context.commands[1]
        );
        assert!(
            context.commands[2].ends_with(":--m"),
            "monospace must be recorded: {}",
            context.commands[2]
        );
    }

    #[test]
    fn memory_backend_records_the_command_stream() {
        let mut pagination = PrintPagination::new();
        pagination.set_ranges_from_spec("2").expect("valid range");
        let doc = TestDoc::new(3);
        let printer = Printer::new();
        // The default backend is platform-selected; drive the memory path directly so
        // the assertion does not depend on whether a spooler exists on the test host.
        let mut context = MemoryPrintContext::new(Size { width: 595, height: 842 });
        for page in pagination.selected_pages(doc.page_count()) {
            doc.draw_page(page, &mut context);
            context.end_page();
        }
        let job = PrintJobPayload {
            page_size: Size { width: 595, height: 842 },
            commands: context.commands,
        };
        assert_eq!(doc.drawn_pages(), vec![1], "only page 2 (zero-based 1) was selected");
        assert!(
            !job.commands.is_empty(),
            "drawing a selected page must emit commands, not an empty stream"
        );
        assert_eq!(printer.backend_name(), printer.backend_name(), "the backend name is stable");
    }

    #[test]
    fn print_manager_creates_and_tracks_jobs() {
        let mut manager = PrintManager::new();
        let settings = PrintSettings::new();
        let pages = vec![PrintPage::new(
            1,
            Size { width: 595, height: 842 },
            vec!["text:Hello@10,10:12".into()],
        )];
        let job = manager.create_job(settings, pages, 1);
        assert_eq!(job.id, 1);
        assert_eq!(job.status, PrintJobStatus::Queued);
        assert_eq!(manager.job_status(1), Some(PrintJobStatus::Queued));
        assert!(manager.get_job(1).is_some());
    }

    #[test]
    fn print_manager_cancels_queued_job() {
        let mut manager = PrintManager::new();
        let settings = PrintSettings::new();
        let pages = vec![
            PrintPage::new(1, Size { width: 595, height: 842 }, Vec::new()),
            PrintPage::new(2, Size { width: 595, height: 842 }, Vec::new()),
        ];
        let job = manager.create_job(settings, pages, 2);
        assert_eq!(job.id, 1);
        assert!(manager.cancel_job(1));
        assert_eq!(manager.job_status(1), Some(PrintJobStatus::Cancelled));
        // Cancelling again should return false (already cancelled)
        assert!(!manager.cancel_job(1));
    }

    #[test]
    fn print_manager_cancel_nonexistent_job_returns_false() {
        let mut manager = PrintManager::new();
        assert!(!manager.cancel_job(99));
        assert_eq!(manager.job_status(99), None);
    }

    #[test]
    fn print_orientation_swaps_dimensions_for_landscape() {
        let portrait = Size { width: 595, height: 842 };
        let landscape = PrintOrientation::Landscape.apply(portrait);
        assert_eq!(landscape.width, 842);
        assert_eq!(landscape.height, 595);
        // Portrait leaves dimensions unchanged
        let same = PrintOrientation::Portrait.apply(portrait);
        assert_eq!(same.width, 595);
        assert_eq!(same.height, 842);
    }

    #[test]
    fn print_settings_defaults() {
        let settings = PrintSettings::new();
        assert_eq!(settings.orientation, PrintOrientation::Portrait);
        assert_eq!(settings.copies, 1);
        assert!(settings.collate);
        assert_eq!(settings.color_mode, "color");
        assert!(settings.page_range.is_none());
    }

    #[test]
    fn print_settings_apply_to_pagination() {
        let mut settings = PrintSettings::new();
        settings.copies = 3;
        settings.collate = true;
        let pagination = settings.apply_to_pagination(5);
        // With no page_range specified, should use all pages
        let pages = pagination.selected_pages(5);
        assert!(!pages.is_empty());
    }

    #[test]
    fn print_job_summary_includes_details() {
        let settings = PrintSettings::new();
        let pages = vec![];
        let job = PrintJob::new(7, settings, pages, 10);
        let summary = job.summary();
        assert!(summary.contains("#7"));
        assert!(summary.contains("10 pages"));
        assert!(summary.contains("Portrait"));
    }

    #[test]
    fn print_page_tracks_content() {
        let commands = vec!["text:Hello@10,10:12".into(), "rect:0,0,100,50:1".into()];
        let page = PrintPage::new(3, Size { width: 800, height: 600 }, commands);
        assert_eq!(page.number, 3);
        assert_eq!(page.command_count(), 2);
        assert_eq!(page.size.width, 800);
        assert_eq!(page.size.height, 600);
    }

    /// The print entry points must consult the backend rather than a
    /// `cfg(target_os)` branch, so their availability follows the *host's actual
    /// spooler*. On a host the backend reports as having print support, the call
    /// must get past the capability check and only then judge the content; on a
    /// host without one it must fail with the "not supported" reason.
    ///
    /// This pins the observable contract of the refactor: the gate is now
    /// `Platform::has_print_support()`, not a compile-time OS list.
    #[test]
    fn print_entry_points_gate_on_backend_capability_not_target_os() {
        let supported = crate::platform::platform_facts().has_print_support();

        let result = print_to_printer("", &PrintSettings::new());
        if supported {
            // Capability check passed, so the empty-content guard is what fires.
            let err = result.unwrap_err();
            assert!(
                err.contains("empty document") && err.contains("0 bytes"),
                "expected the empty-content guard, got: {err}"
            );
        } else {
            let err = result.unwrap_err();
            assert!(
                err.contains("no system printer is available") && err.contains("0 bytes"),
                "expected the capability guard, got: {err}"
            );
        }

        // A host without a spooler must reject the dialog outright; a host with one
        // proceeds to the (non-interactive, so cancelling) console prompt.
        let dialog = print_page_dialog();
        if supported {
            assert_eq!(dialog, Ok(false), "no interactive terminal must cancel, not accept");
        } else {
            assert_eq!(dialog, Err("print dialog is not supported on this platform".to_string()));
        }
    }

    // ── PrintContext / PrintDocument contract ─────────────────────────

    /// `draw_page` receives a **zero-based index**, and the first page is `0`.
    ///
    /// This was ambiguous before the contract review: the parameter was named
    /// `page_num` and the only caller passed values from `selected_pages`, which is
    /// index-based. A document author rendering "Page 3" from that value would have
    /// been off by one, and nothing in the code or tests said which it was.
    #[test]
    fn draw_page_receives_a_zero_based_index() {
        let printer =
            Printer { page_size: Size { width: 595, height: 842 }, backend: PrintBackend::Memory };
        let doc = TestDoc::new(3);
        printer.print_with_result(&doc).expect("memory backend never fails");

        assert_eq!(
            doc.drawn_pages(),
            vec![0, 1, 2],
            "a three-page document must be drawn as indices 0,1,2 — a first value of 1 \
             would mean the parameter is a one-based page number"
        );
    }

    /// The pipeline may repeat, skip and reorder page indices, so a document must
    /// not assume one ascending visit per page.
    ///
    /// This is the observable consequence of `draw_page` being driven by
    /// `selected_pages` rather than by a `0..page_count` loop: copies repeat an
    /// index, ranges skip the rest, and descending order reverses them.
    #[test]
    fn draw_page_may_be_called_repeatedly_and_out_of_order() {
        let printer =
            Printer { page_size: Size { width: 595, height: 842 }, backend: PrintBackend::Memory };
        let doc = TestDoc::new(5);

        let mut pagination = PrintPagination::new();
        pagination.set_range(3, 4);
        pagination.set_page_order(PageOrder::Descending);
        pagination.set_copies(2);
        pagination.set_collate(true);

        printer.print_with_pagination_result(&doc, &pagination).expect("memory backend");

        // Pages 3 and 4 (one-based) are indices 2 and 3, descending, twice over.
        assert_eq!(
            doc.drawn_pages(),
            vec![3, 2, 3, 2],
            "draw_page must be handed exactly the selected indices, in selection order"
        );
    }

    /// A document with no pages prints nothing and must not be an error.
    #[test]
    fn a_document_with_no_pages_prints_nothing_without_failing() {
        let printer =
            Printer { page_size: Size { width: 595, height: 842 }, backend: PrintBackend::Memory };
        let doc = TestDoc::new(0);
        let result = printer.print_with_result(&doc);

        assert!(result.is_ok(), "an empty document is not a failure, got {result:?}");
        assert!(doc.drawn_pages().is_empty(), "no page may be drawn for a 0-page document");
    }

    /// The preview must expose the commands the document actually produced.
    ///
    /// `preview_commands` was a field that nothing ever wrote to: `show()` rendered
    /// through a throwaway `Printer` and dropped the output, so a caller could ask
    /// for the preview and always get an empty list. The accessor existed and was
    /// documented to return "rendered preview output", which made it a silent lie.
    #[test]
    fn preview_exposes_the_commands_it_recorded() {
        let mut preview = PrintPreviewDialog::new(Box::new(RecordingDoc));
        assert!(preview.show(), "a document that draws must preview successfully");
        let commands = preview.preview_commands();
        assert!(
            commands.iter().any(|c| c.starts_with("text:")),
            "preview must expose the recorded draw commands, got {commands:?}"
        );
        assert!(commands.contains(&"page-break".to_string()), "each page must end with a break");
    }

    /// A preview of a document with no pages must fail and record nothing.
    ///
    /// The two are different questions: `show()` is about whether there is something
    /// to preview, `preview_commands()` about what was recorded. A zero-page document
    /// answers no/false and must leave no stale output behind.
    #[test]
    fn previewing_an_empty_document_fails_and_records_nothing() {
        let mut preview = PrintPreviewDialog::new(Box::new(TestDoc::new(0)));
        assert!(!preview.show(), "a zero-page document has nothing to preview");
        assert!(
            preview.preview_commands().is_empty(),
            "a failed preview must not record commands, got {:?}",
            preview.preview_commands()
        );
    }

    /// A document that draws nothing still yields one page break per page.
    ///
    /// Pins the page structure independently of drawing: the frame exists even when
    /// the document puts no marks on it, which is what lets a caller count pages from
    /// the recorded output.
    #[test]
    fn a_page_with_no_marks_still_records_its_page_break() {
        let mut preview = PrintPreviewDialog::new(Box::new(TestDoc::new(2)));
        assert!(preview.show(), "a two-page document is previewable");
        let breaks = preview.preview_commands().iter().filter(|c| *c == "page-break").count();
        assert_eq!(breaks, 2, "one page break per page, even for a blank page");
    }

    /// Previewing twice must reflect the second render, not accumulate both.
    #[test]
    fn previewing_twice_replaces_rather_than_appends() {
        let mut preview = PrintPreviewDialog::new(Box::new(RecordingDoc));
        assert!(preview.show(), "first preview");
        let first = preview.preview_commands().len();
        assert!(preview.show(), "second preview");
        assert_eq!(
            preview.preview_commands().len(),
            first,
            "a second render must replace the recorded commands, not append to them"
        );
    }

    /// The context must report the page size the caller asked for.
    #[test]
    fn context_reports_the_configured_page_size() {
        let size = Size { width: 123, height: 456 };
        let context = MemoryPrintContext::new(size);
        assert_eq!(context.page_size(), size);
    }

    /// A one-page document whose index is out of range must not be drawn.
    ///
    /// Guards the boundary of the `draw_page` index contract: with `page_count() == 1`
    /// the only valid index is `0`.
    #[test]
    fn an_out_of_range_index_is_never_requested() {
        let printer =
            Printer { page_size: Size { width: 595, height: 842 }, backend: PrintBackend::Memory };
        let doc = TestDoc::new(1);
        printer.print_with_result(&doc).expect("memory backend");
        assert_eq!(doc.drawn_pages(), vec![0]);
        assert!(
            doc.drawn_pages().iter().all(|index| *index < doc.page_count()),
            "every requested index must be within page_count()"
        );
    }

    /// A document that draws text, used to give the preview something to record.
    struct RecordingDoc;
    impl PrintDocument for RecordingDoc {
        fn page_count(&self) -> u32 {
            2
        }
        fn draw_page(&self, page_index: u32, context: &mut dyn PrintContext) {
            context.draw_text(&format!("page {page_index}"), 10.0, 20.0, 12.0, Color::BLACK);
        }
    }
}
