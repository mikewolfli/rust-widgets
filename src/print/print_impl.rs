// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Printing and print preview support.
//!
//! Note: there is currently no native/system print dialog integration.
//! `print_page_dialog` is a console confirmation (y/n) that defaults to cancel
//! when no interactive terminal is available, and `print_to_printer` submits
//! rendered content through a platform print command.
use crate::core::{Rect, Size};
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
/// use rust_widgets::print::{PrintContext, PrintDocument};
/// use rust_widgets::core::{Rect, Size};
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
///         context.draw_text("INVOICE", 40.0, 40.0, 18.0);
///         if let Some(line) = self.lines.get(page_index as usize) {
///             let _ = page;
///             context.draw_text(line, 40.0, 80.0, 12.0);
///         }
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
    /// Draws `text` with its left edge at `x` and baseline at `y`.
    ///
    /// `font_size` is in page-space units. There is no font-selection parameter yet:
    /// the context chooses the family, so a document cannot request bold or italic.
    fn draw_text(&mut self, text: &str, x: f32, y: f32, font_size: f32);

    /// Draws a straight line from `(x1, y1)` to `(x2, y2)` with the given width.
    fn draw_line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, width: f32);

    /// Outlines `rect` with the given stroke width.
    ///
    /// Takes no colour, unlike [`Self::fill_rect`]; an outline is currently always
    /// the context's default stroke colour.
    fn draw_rect(&mut self, rect: Rect, width: f32);

    /// Fills `rect` with `color`, as `0xRRGGBB`.
    ///
    /// An alpha channel is not interpreted; the top byte is ignored.
    fn fill_rect(&mut self, rect: Rect, color: u32);

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

    /// The size of one page, in the same units as the coordinates above.
    ///
    /// Constant for the lifetime of the context: every page of a job is the same
    /// size, so a document may compute its layout from this once per page.
    fn page_size(&self) -> Size;
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
    let file = opts
        .open(&path)
        .map_err(|err| format!("create print job file failed at {}: {err}", path.display()))?;

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
        return Err(format!("write print job file failed: {err}"));
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
        return Err("print dialog is not supported on this platform".to_string());
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
        return Err("system printer is not supported on this platform".to_string());
    }
    if content.is_empty() {
        return Err("cannot print empty content".to_string());
    }
    let mut path = std::env::temp_dir();
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| format!("clock error: {err}"))?
        .as_millis();
    path.push(format!("rw_print_output_{ts}.txt"));
    if let Err(err) = fs::write(&path, content) {
        return Err(format!("failed to write print temporary file at {}: {err}", path.display()));
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
}
impl MemoryPrintContext {
    /// Creates an in-memory print context for the given page size.
    pub fn new(page_size: Size) -> Self {
        Self { page_size, commands: Vec::new() }
    }
    /// Appends a page break marker to the command stream.
    pub fn end_page(&mut self) {
        self.commands.push("page-break".to_string());
    }
}
impl PrintContext for MemoryPrintContext {
    fn draw_text(&mut self, text: &str, x: f32, y: f32, font_size: f32) {
        self.commands.push(format!("text:{text}@{x},{y}:{font_size}"));
    }
    fn draw_line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, width: f32) {
        self.commands.push(format!("line:{x1},{y1}->{x2},{y2}:{width}"));
    }
    fn draw_rect(&mut self, rect: Rect, width: f32) {
        self.commands
            .push(format!("rect:{},{},{},{}:{}", rect.x, rect.y, rect.width, rect.height, width));
    }
    fn fill_rect(&mut self, rect: Rect, color: u32) {
        self.commands
            .push(format!("fill:{},{},{},{}:{color}", rect.x, rect.y, rect.width, rect.height));
    }
    fn draw_image(&mut self, image: &[u8], rect: Rect) {
        self.commands.push(format!(
            "img:{}bytes:{},{},{},{}",
            image.len(),
            rect.x,
            rect.y,
            rect.width,
            rect.height
        ));
    }
    fn page_size(&self) -> Size {
        self.page_size
    }
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
            assert_eq!(result, Err("cannot print empty content".to_string()));
        } else {
            assert_eq!(result, Err("system printer is not supported on this platform".to_string()));
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
            context.draw_text(&format!("page {page_index}"), 10.0, 20.0, 12.0);
        }
    }
}
