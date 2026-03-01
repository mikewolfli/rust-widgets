//! Printing and print preview support.

use crate::core::{Rect, Size};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

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
}

impl PrintDialog {
    pub fn new() -> Self {
        Self { copies: 1 }
    }

    pub fn set_copies(&mut self, copies: u32) {
        self.copies = copies.max(1);
    }

    pub fn show(&self) -> bool {
        self.copies >= 1
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
}

impl PrintPreviewDialog {
    pub fn new(document: Box<dyn PrintDocument>) -> Self {
        Self {
            page_count: document.page_count(),
            current_page: 0,
        }
    }

    pub fn page_count(&self) -> u32 {
        self.page_count
    }

    pub fn current_page(&self) -> u32 {
        self.current_page
    }

    pub fn next_page(&mut self) {
        if self.current_page + 1 < self.page_count {
            self.current_page += 1;
        }
    }

    pub fn prev_page(&mut self) {
        self.current_page = self.current_page.saturating_sub(1);
    }

    pub fn show(&self) -> bool {
        self.page_count > 0
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
    pub fn new() -> Self {
        Self {
            page_size: Size {
                width: 595,
                height: 842,
            },
            backend: PrintBackend::default_for_platform(),
        }
    }

    pub fn print(&self, document: &dyn PrintDocument) {
        let _ = self.print_with_result(document);
    }

    /// Print and return backend execution result.
    pub fn print_with_result(&self, document: &dyn PrintDocument) -> Result<(), String> {
        let mut context = MemoryPrintContext::new(self.page_size);
        for page in 0..document.page_count() {
            document.draw_page(page, &mut context);
            context.end_page();
        }

        let job = PrintJob {
            page_size: self.page_size,
            commands: context.commands,
        };

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

struct PrintJob {
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

impl PrintBackend {
    fn default_for_platform() -> Self {
        if std::env::var("RUST_WIDGETS_PRINT_BACKEND")
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

    fn submit(&self, job: &PrintJob) -> Result<(), String> {
        match self {
            PrintBackend::System => submit_system_print_job(job),
            PrintBackend::Memory => Ok(()),
        }
    }
}

fn submit_system_print_job(job: &PrintJob) -> Result<(), String> {
    let path = write_print_job_file(job)?;
    let result = run_print_command(&path);
    let _ = fs::remove_file(&path);
    result
}

fn write_print_job_file(job: &PrintJob) -> Result<PathBuf, String> {
    let mut path = std::env::temp_dir();
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| format!("clock error: {err}"))?
        .as_millis();
    path.push(format!("rust_widgets_print_job_{ts}.txt"));

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

fn run_print_command(path: &PathBuf) -> Result<(), String> {
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

        return Err("no available system print command succeeded (tried: lpr, lp)".to_string());
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

        return Err("system print command failed on windows".to_string());
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        let _ = path;
        Err("system print backend is not supported on this platform".to_string())
    }
}

pub struct MemoryPrintContext {
    page_size: Size,
    /// Recorded drawing commands for tests/demos.
    pub commands: Vec<String>,
}

impl MemoryPrintContext {
    pub fn new(page_size: Size) -> Self {
        Self {
            page_size,
            commands: Vec::new(),
        }
    }

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
        self.commands.push(format!("rect:{},{},{},{}:{}", rect.x, rect.y, rect.width, rect.height, width));
    }

    fn fill_rect(&mut self, rect: Rect, color: u32) {
        self.commands.push(format!("fill:{},{},{},{}:{color}", rect.x, rect.y, rect.width, rect.height));
    }

    fn draw_image(&mut self, image: &[u8], rect: Rect) {
        self.commands.push(format!("img:{}bytes:{},{},{},{}", image.len(), rect.x, rect.y, rect.width, rect.height));
    }

    fn page_size(&self) -> Size {
        self.page_size
    }
}
