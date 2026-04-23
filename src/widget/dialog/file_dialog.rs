//! File dialog widget.
use crate::core::{Color, Font, ObjectId, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{ConnectionScope, GenericSignal, Signal1};
use crate::style::WidgetStyle;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
/// File dialog mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileDialogMode {
    OpenFile,
    OpenFiles,
    SaveFile,
    SelectDirectory,
}
/// File name filter entry.
#[derive(Debug, Clone)]
pub struct FileFilter {
    pub description: String,
    pub extensions: Vec<String>,
}
impl FileFilter {
    pub fn new(description: impl Into<String>, extensions: Vec<impl Into<String>>) -> Self {
        Self {
            description: description.into(),
            extensions: extensions.into_iter().map(|e| e.into()).collect(),
        }
    }
    pub fn all_files() -> Self {
        Self::new("All Files (*)", vec!["*"])
    }
}
impl std::fmt::Display for FileFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let exts: Vec<String> = self.extensions.iter().map(|e| format!("*.{}", e)).collect();
        write!(f, "{} ({})", self.description, exts.join(" "))
    }
}
/// File dialog widget.
pub struct FileDialog {
    base: BaseWidget,
    mode: FileDialogMode,
    title: String,
    directory: String,
    selected_files: Vec<String>,
    name_filters: Vec<FileFilter>,
    current_filter: usize,
    pub files_selected: Signal1<Vec<String>>,
    pub file_selected: Signal1<String>,
    pub current_changed: Signal1<String>,
    pub accepted: GenericSignal,
    pub rejected: GenericSignal,
}
impl FileDialog {
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Dialog, geometry, "FileDialog"),
            mode: FileDialogMode::OpenFile,
            title: "Open File".to_string(),
            directory: String::new(),
            selected_files: Vec::new(),
            name_filters: vec![FileFilter::all_files()],
            current_filter: 0,
            files_selected: Signal1::new(),
            file_selected: Signal1::new(),
            current_changed: Signal1::new(),
            accepted: GenericSignal::new(),
            rejected: GenericSignal::new(),
        }
    }
    pub fn open_file(geometry: Rect) -> Self {
        let mut d = Self::new(geometry);
        d.mode = FileDialogMode::OpenFile;
        d.title = "Open File".to_string();
        d
    }
    pub fn save_file(geometry: Rect) -> Self {
        let mut d = Self::new(geometry);
        d.mode = FileDialogMode::SaveFile;
        d.title = "Save File".to_string();
        d
    }
    pub fn mode(&self) -> FileDialogMode {
        self.mode
    }
    pub fn title(&self) -> &str {
        &self.title
    }
    pub fn directory(&self) -> &str {
        &self.directory
    }
    pub fn selected_files(&self) -> &[String] {
        &self.selected_files
    }
    pub fn selected_file(&self) -> Option<&str> {
        self.selected_files.first().map(|s| s.as_str())
    }
    pub fn name_filters(&self) -> &[FileFilter] {
        &self.name_filters
    }
    pub fn current_filter(&self) -> Option<&FileFilter> {
        self.name_filters.get(self.current_filter)
    }
    pub fn set_mode(&mut self, mode: FileDialogMode) {
        self.mode = mode;
        self.title = match mode {
            FileDialogMode::OpenFile | FileDialogMode::OpenFiles => "Open File",
            FileDialogMode::SaveFile => "Save File",
            FileDialogMode::SelectDirectory => "Select Directory",
        }
        .to_string();
    }
    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title = title.into();
    }
    pub fn set_directory(&mut self, dir: impl Into<String>) {
        self.directory = dir.into();
    }
    pub fn set_name_filters(&mut self, filters: Vec<FileFilter>) {
        self.name_filters = filters;
        self.current_filter = 0;
    }
    pub fn select_file(&mut self, path: impl Into<String>) {
        let path = path.into();
        self.selected_files = vec![path.clone()];
        self.file_selected.emit(path);
    }
    pub fn accept(&mut self) {
        if !self.selected_files.is_empty() {
            self.files_selected.emit(self.selected_files.clone());
        }
        self.accepted.emit();
        self.hide();
    }
    pub fn reject(&mut self) {
        self.selected_files.clear();
        self.rejected.emit();
        self.hide();
    }
}
impl Widget for FileDialog {
    fn id(&self) -> ObjectId {
        self.base.id()
    }
    fn kind(&self) -> WidgetKind {
        self.base.kind()
    }
    fn geometry(&self) -> Rect {
        self.base.geometry()
    }
    fn set_geometry(&mut self, g: Rect) {
        self.base.set_geometry(g);
    }
    fn min_size(&self) -> Option<Size> {
        self.base.min_size()
    }
    fn max_size(&self) -> Option<Size> {
        self.base.max_size()
    }
    fn set_min_size(&mut self, s: Option<Size>) {
        self.base.set_min_size(s);
    }
    fn set_max_size(&mut self, s: Option<Size>) {
        self.base.set_max_size(s);
    }
    fn parent(&self) -> Option<ObjectId> {
        self.base.parent()
    }
    fn set_parent(&mut self, p: Option<ObjectId>) {
        self.base.set_parent(p);
    }
    fn add_child(&mut self, c: ObjectId) {
        self.base.add_child(c);
    }
    fn remove_child(&mut self, c: ObjectId) {
        self.base.remove_child(c);
    }
    fn children(&self) -> &[ObjectId] {
        self.base.children()
    }
    fn show(&mut self) {
        self.base.show();
    }
    fn hide(&mut self) {
        self.base.hide();
    }
    fn is_visible(&self) -> bool {
        self.base.is_visible()
    }
    fn set_enabled(&mut self, e: bool) {
        self.base.set_enabled(e);
    }
    fn is_enabled(&self) -> bool {
        self.base.is_enabled()
    }
    fn set_tooltip(&mut self, t: String) {
        self.base.set_tooltip(t);
    }
    fn tooltip(&self) -> &str {
        self.base.tooltip()
    }
    fn style(&self) -> &WidgetStyle {
        self.base.style()
    }
    fn set_style(&mut self, s: WidgetStyle) {
        self.base.set_style(s);
    }
    fn connection_scope(&self) -> &ConnectionScope {
        self.base.connection_scope()
    }
    fn hover_signal(&self) -> &Signal1<Point> {
        self.base.hover_signal()
    }
    fn mouse_down_signal(&self) -> &Signal1<(Point, u32)> {
        self.base.mouse_down_signal()
    }
    fn mouse_up_signal(&self) -> &Signal1<(Point, u32)> {
        self.base.mouse_up_signal()
    }
    fn key_down_signal(&self) -> &Signal1<(u32, u32)> {
        self.base.key_down_signal()
    }
    fn key_up_signal(&self) -> &Signal1<(u32, u32)> {
        self.base.key_up_signal()
    }
    fn focus_gained_signal(&self) -> &GenericSignal {
        self.base.focus_gained_signal()
    }
    fn focus_lost_signal(&self) -> &GenericSignal {
        self.base.focus_lost_signal()
    }
    fn redraw_requested_signal(&self) -> &GenericSignal {
        self.base.redraw_requested_signal()
    }
    fn layout_requested_signal(&self) -> &GenericSignal {
        self.base.layout_requested_signal()
    }
}
impl EventHandler for FileDialog {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::KeyPress { key, .. } => {
                if *key == 13 {
                    self.accept();
                } else if *key == 27 {
                    self.reject();
                }
            }
            _ => {}
        }
    }
}
impl Draw for FileDialog {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        context.fill_rect(
            Rect::new(rect.x, rect.y, rect.width, rect.height),
            Color::from_rgb(245, 245, 245),
        );
        context.draw_rect(
            Rect::new(rect.x, rect.y, rect.width, rect.height),
            Color::from_rgb(160, 160, 160),
        );
        context.fill_rect(
            Rect::new(rect.x, rect.y, rect.width, 28),
            Color::from_rgb(0, 120, 215),
        );
        context.draw_text(
            Point::new(rect.x + 8, rect.y + 14),
            &self.title,
            &Font::default(),
            Color::from_rgb(255, 255, 255),
        );
        // File list area
        let list_y = rect.y + 38;
        let list_h = rect.height.saturating_sub(120);
        context.fill_rect(
            Rect::new(rect.x + 10, list_y, rect.width.saturating_sub(20), list_h),
            Color::from_rgb(255, 255, 255),
        );
        context.draw_rect(
            Rect::new(rect.x + 10, list_y, rect.width.saturating_sub(20), list_h),
            Color::from_rgb(150, 150, 150),
        );
        context.draw_text(
            Point::new(rect.x + 16, list_y + 20),
            "(file list)",
            &Font::default(),
            Color::from_rgb(150, 150, 150),
        );
        // Selected files display
        let sel_y = list_y + list_h as i32 + 8;
        context.draw_text(
            Point::new(rect.x + 10, sel_y + 10),
            "File name:",
            &Font::default(),
            Color::from_rgb(0, 0, 0),
        );
        let fname = self.selected_file().unwrap_or("");
        context.fill_rect(
            Rect::new(rect.x + 80, sel_y, rect.width.saturating_sub(90), 22),
            Color::from_rgb(255, 255, 255),
        );
        context.draw_rect(
            Rect::new(rect.x + 80, sel_y, rect.width.saturating_sub(90), 22),
            Color::from_rgb(150, 150, 150),
        );
        context.draw_text(
            Point::new(rect.x + 84, sel_y + 11),
            fname,
            &Font::default(),
            Color::from_rgb(0, 0, 0),
        );
        // OK/Cancel buttons
        let btn_y = rect.y as f32 + rect.height as f32 - 40.0;
        let btn_w = 80;
        let ok_label = if self.mode == FileDialogMode::SaveFile {
            "Save"
        } else {
            "Open"
        };
        context.fill_rect(
            Rect::new(rect.x + rect.width as i32 - 176, btn_y as i32, btn_w, 28),
            Color::from_rgb(0, 120, 215),
        );
        context.draw_text(
            Point::new(rect.x + rect.width as i32 - 136, (btn_y + 14.0) as i32),
            ok_label,
            &Font::default(),
            Color::from_rgb(255, 255, 255),
        );
        context.fill_rect(
            Rect::new(rect.x + rect.width as i32 - 88, btn_y as i32, btn_w, 28),
            Color::from_rgb(225, 225, 225),
        );
        context.draw_rect(
            Rect::new(rect.x + rect.width as i32 - 88, btn_y as i32, btn_w, 28),
            Color::from_rgb(100, 100, 100),
        );
        context.draw_text(
            Point::new(rect.x + rect.width as i32 - 48, (btn_y + 14.0) as i32),
            "Cancel",
            &Font::default(),
            Color::from_rgb(0, 0, 0),
        );
    }
}
