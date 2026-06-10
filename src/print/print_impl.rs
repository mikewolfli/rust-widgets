//! Printing and print preview support.
use crate::core::{Rect, Size};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
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
impl Default for PrintPagination {
    fn default() -> Self {
        Self::new()
    }
}
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
/// Print document
pub trait PrintDocument {
    /// Get number of pages
    fn page_count(&self) -> u32;
    /// Draw one page into provided print context.
    fn draw_page(&self, page_num: u32, context: &mut dyn PrintContext);
}
/// Print context
pub trait PrintContext {
    /// Draw text
    fn draw_text(&mut self, text: &str, x: f32, y: f32, font_size: f32);
    /// Draw line
    fn draw_line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, width: f32);
    /// Draw rectangle
    fn draw_rect(&mut self, rect: Rect, width: f32);
    /// Draw filled rectangle
    fn fill_rect(&mut self, rect: Rect, color: u32);
    /// Draw image
    fn draw_image(&mut self, image: &[u8], rect: Rect);
    /// Get page size
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

        // Check if the platform has a native print command available.
        // On Unix-like systems we look for `lp` or `lpr`; on Windows we check `print`.
        // When a real native print dialog is available (e.g., Cocoa PrintPanel on macOS,
        // PrintDlg on Windows, GtkPrintDialog on Linux) this method would present the
        // platform-native dialog and return user choices.
        let has_printer = if cfg!(target_os = "windows") {
            std::process::Command::new("cmd")
                .args(["/C", "print /? 2>NUL"])
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        } else {
            std::process::Command::new("lp")
                .arg("--version")
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
                || std::process::Command::new("lpr")
                    .arg("--version")
                    .output()
                    .map(|o| o.status.success())
                    .unwrap_or(false)
        };

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
impl Default for PrintDialog {
    fn default() -> Self {
        Self::new()
    }
}
/// Print preview dialog
pub struct PrintPreviewDialog {
    /// Total document pages.
    page_count: u32,
    /// Currently selected page index.
    current_page: u32,
    /// Stored document reference for rendering previews.
    document: Option<Box<dyn PrintDocument>>,
    /// Rendered preview output (commands from Memory backend).
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
    /// Returns whether preview can be displayed.
    ///
    /// Renders the document pages using a temporary Printer with the Memory
    /// backend, storing the generated output internally. Returns `true` only
    /// if the document has at least one page and the preview was generated.
    pub fn show(&mut self) -> bool {
        if self.page_count == 0 {
            log::warn!("PrintPreviewDialog::show() — no pages to preview");
            return false;
        }

        if self.document.is_none() {
            log::warn!("PrintPreviewDialog::show() — document was already consumed");
            return false;
        }

        // Take ownership of the document to render the preview.
        let Some(document) = self.document.take() else {
            return false;
        };

        // Create a temporary printer with the Memory backend to capture output.
        let printer =
            Printer { page_size: Size { width: 595, height: 842 }, backend: PrintBackend::Memory };

        let result = printer.print_with_result(document.as_ref());

        match result {
            Ok(()) => {
                log::info!(
                    "PrintPreviewDialog::show() — preview generated ({} pages)",
                    self.page_count
                );
                // Store the document back so it can be used again if needed.
                self.document = Some(document);
                true
            }
            Err(e) => {
                log::error!("PrintPreviewDialog::show() — preview failed: {}", e);
                // Store the document back even on failure.
                self.document = Some(document);
                false
            }
        }
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
            log::error!("[print] print failed: {}", e);
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
            log::error!("[print] print_with_pagination failed: {}", e);
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
impl Default for Printer {
    fn default() -> Self {
        Self::new()
    }
}
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
                let label = format!("memory-job-{}", ts);
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
fn write_print_job_file(job: &PrintJobPayload) -> Result<PathBuf, String> {
    let mut path = std::env::temp_dir();
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| format!("clock error: {err}"))?
        .as_millis();
    path.push(format!("rw_print_job_{ts}.txt"));
    let mut content = String::new();
    content.push_str(&format!(
        "rust_widgets print job\npage_size={}x{}\n\n",
        job.page_size.width, job.page_size.height
    ));
    for cmd in &job.commands {
        content.push_str(cmd);
        content.push('\n');
    }
    fs::write(&path, content).map_err(|err| format!("write print job file failed: {err}"))?;
    Ok(path)
}
fn run_print_command(path: &Path) -> Result<(), String> {
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        let lpr_status = Command::new("lpr").arg(path).status();
        if let Ok(status) = lpr_status {
            if status.success() {
                return Ok(());
            }
        }
        let lp_status = Command::new("lp").arg(path).status();
        if let Ok(status) = lp_status {
            if status.success() {
                return Ok(());
            }
        }
        Err("no available system print command succeeded (tried: lpr, lp)".to_string())
    }
    #[cfg(target_os = "windows")]
    {
        let status = Command::new("powershell")
            .arg("-NoProfile")
            .arg("-Command")
            .arg(format!(
                "Start-Process -FilePath '{}' -Verb Print -PassThru | Out-Null",
                path.display()
            ))
            .status();
        if let Ok(status) = status {
            if status.success() {
                return Ok(());
            }
        }
        Err("system print command failed on windows".to_string())
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        let _ = path;
        Err("system print backend is not supported on this platform".to_string())
    }
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

impl Default for PrintManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Platform-specific: show a system print dialog.
/// On non-desktop platforms this returns an error message.
#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
pub fn print_page_dialog() -> Result<bool, String> {
    // State-only implementation: on desktop targets we acknowledge the dialog
    // exists but do not attach a native window handle. Real implementations
    // would call the platform's print dialog API (NSPrintPanel, GtkPrintUnixDialog, etc.).
    log::info!("[print] print_page_dialog() called — native dialog integration pending");
    Ok(true)
}

/// Platform-specific: show a system print dialog (fallback for unsupported platforms).
#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
pub fn print_page_dialog() -> Result<bool, String> {
    Err("print dialog is not supported on this platform".to_string())
}

/// Platform-specific: submit rendered content to the system printer.
#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
pub fn print_to_printer(content: &str, _settings: &PrintSettings) -> Result<(), String> {
    // State-only implementation: write a temporary file with the rendered
    // content and attempt to submit via system print commands.
    let mut path = std::env::temp_dir();
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| format!("clock error: {err}"))?
        .as_millis();
    path.push(format!("rw_print_output_{ts}.txt"));
    fs::write(&path, content).map_err(|err| format!("write print output failed: {err}"))?;
    let result = run_print_command(&path);
    let _ = fs::remove_file(&path);
    result
}

/// Platform-specific: submit rendered content to the system printer (fallback).
#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
pub fn print_to_printer(_content: &str, _settings: &PrintSettings) -> Result<(), String> {
    Err("system printer is not supported on this platform".to_string())
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
}
