//! File dialog widget.
use crate::core::{HorizontalAlignment, Color, Font, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::tr;

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
        Self::new(tr!("dialog.file_dialog.all_files_filter"), vec!["*"])
    }
}
impl std::fmt::Display for FileFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let exts: Vec<String> = self.extensions.iter().map(|e| format!("*.{e}")).collect();
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
    modal: bool,
    pub files_selected: Signal1<Vec<String>>,
    pub file_selected: Signal1<String>,
    pub current_changed: Signal1<String>,
    pub accepted: GenericSignal,
    pub rejected: GenericSignal,
}
impl FileDialog {
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::FileDialog, geometry, "FileDialog"),
            mode: FileDialogMode::OpenFile,
            title: tr!("dialog.file_dialog.open_file"),
            directory: String::new(),
            selected_files: Vec::new(),
            name_filters: vec![FileFilter::all_files()],
            current_filter: 0,
            modal: true,
            files_selected: Signal1::new(),
            file_selected: Signal1::new(),
            current_changed: Signal1::new(),
            accepted: GenericSignal::new(),
            rejected: GenericSignal::new(),
        }
    }
    pub fn is_modal(&self) -> bool {
        self.modal
    }
    pub fn set_modal(&mut self, modal: bool) {
        self.modal = modal;
        self.base.request_redraw();
    }
    pub fn open_file(geometry: Rect) -> Self {
        let mut d = Self::new(geometry);
        d.mode = FileDialogMode::OpenFile;
        d.title = tr!("dialog.file_dialog.open_file");
        d
    }
    pub fn save_file(geometry: Rect) -> Self {
        let mut d = Self::new(geometry);
        d.mode = FileDialogMode::SaveFile;
        d.title = tr!("dialog.file_dialog.save_file");
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
        self.title = tr!(match mode {
            FileDialogMode::OpenFile | FileDialogMode::OpenFiles => "dialog.file_dialog.open_file",
            FileDialogMode::SaveFile => "dialog.file_dialog.save_file",
            FileDialogMode::SelectDirectory => "dialog.file_dialog.select_directory",
        });
        self.base.request_redraw();
    }
    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title = title.into();
        self.base.request_redraw();
    }
    pub fn set_directory(&mut self, dir: impl Into<String>) {
        self.directory = dir.into();
        self.base.request_redraw();
    }
    pub fn set_name_filters(&mut self, filters: Vec<FileFilter>) {
        self.name_filters = filters;
        self.current_filter = 0;
        self.base.request_redraw();
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
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        crate::core::Size::new(500, 400)
    }
}
impl EventHandler for FileDialog {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        if let Event::KeyPress { key, .. } = event {
            if *key == 13 {
                self.accept();
            } else if *key == 27 {
                self.reject();
            }
        }
    }
}
impl Draw for FileDialog {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        context.fill_rect(
            Rect::new(rect.x, rect.y, rect.width, rect.height),
            Color::rgb(245, 245, 245),
        );
        context.draw_rect(
            Rect::new(rect.x, rect.y, rect.width, rect.height),
            Color::rgb(160, 160, 160),
        );
        context.fill_rect(Rect::new(rect.x, rect.y, rect.width, 28), Color::rgb(0, 120, 215));
        context.draw_text(
            Point::new(rect.x + 8, rect.y + 14),
            &self.title,
            &Font::default(),
            Color::rgb(255, 255, 255),
            HorizontalAlignment::Left,
        );
        // File list area
        let list_y = rect.y + 38;
        let list_h = rect.height.saturating_sub(120);
        context.fill_rect(
            Rect::new(rect.x + 10, list_y, rect.width.saturating_sub(20), list_h),
            Color::rgb(255, 255, 255),
        );
        context.draw_rect(
            Rect::new(rect.x + 10, list_y, rect.width.saturating_sub(20), list_h),
            Color::rgb(150, 150, 150),
        );
        context.draw_text(
            Point::new(rect.x + 16, list_y + 20),
            &tr!("dialog.file_dialog.file_list_placeholder"),
            &Font::default(),
            Color::rgb(150, 150, 150),
            HorizontalAlignment::Left,
        );
        // Selected files display
        let sel_y = list_y + list_h as i32 + 8;
        context.draw_text(
            Point::new(rect.x + 10, sel_y + 10),
            &tr!("dialog.file_dialog.file_name"),
            &Font::default(),
            Color::rgb(0, 0, 0),
            HorizontalAlignment::Left,
        );
        let fname = self.selected_file().unwrap_or("");
        context.fill_rect(
            Rect::new(rect.x + 80, sel_y, rect.width.saturating_sub(90), 22),
            Color::rgb(255, 255, 255),
        );
        context.draw_rect(
            Rect::new(rect.x + 80, sel_y, rect.width.saturating_sub(90), 22),
            Color::rgb(150, 150, 150),
        );
        context.draw_text(
            Point::new(rect.x + 84, sel_y + 11),
            fname,
            &Font::default(),
            Color::rgb(0, 0, 0),
            HorizontalAlignment::Left,
        );
        // OK/Cancel buttons
        let btn_y = rect.y as f32 + rect.height as f32 - 40.0;
        let btn_w = 80;
        let ok_label = if self.mode == FileDialogMode::SaveFile {
            tr!("common.button.save")
        } else {
            tr!("common.button.open")
        };
        context.fill_rect(
            Rect::new(rect.x + rect.width as i32 - 176, btn_y as i32, btn_w, 28),
            Color::rgb(0, 120, 215),
        );
        context.draw_text(
            Point::new(rect.x + rect.width as i32 - 136, (btn_y + 14.0) as i32),
            &ok_label,
            &Font::default(),
            Color::rgb(255, 255, 255),
            HorizontalAlignment::Left,
        );
        context.fill_rect(
            Rect::new(rect.x + rect.width as i32 - 88, btn_y as i32, btn_w, 28),
            Color::rgb(225, 225, 225),
        );
        context.draw_rect(
            Rect::new(rect.x + rect.width as i32 - 88, btn_y as i32, btn_w, 28),
            Color::rgb(100, 100, 100),
        );
        context.draw_text(
            Point::new(rect.x + rect.width as i32 - 48, (btn_y + 14.0) as i32),
            &tr!("common.button.cancel"),
            &Font::default(),
            Color::rgb(0, 0, 0),
            HorizontalAlignment::Left,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::Event;
    use std::sync::{Arc, Mutex};

    #[test]
    fn select_file_updates_selection_and_emits_signal() {
        let mut dialog = FileDialog::new(Rect::new(0, 0, 420, 280));
        let selected = Arc::new(Mutex::new(String::new()));
        let selected_clone = Arc::clone(&selected);

        dialog.file_selected.connect(move |path| {
            if let Ok(mut v) = selected_clone.lock() {
                *v = (*path).clone();
            }
        });

        dialog.select_file("/tmp/demo.txt");
        assert_eq!(dialog.selected_file(), Some("/tmp/demo.txt"));
        assert_eq!(*selected.lock().expect("selected lock"), "/tmp/demo.txt");
    }

    #[test]
    fn enter_accepts_and_escape_rejects() {
        let mut dialog = FileDialog::new(Rect::new(0, 0, 420, 280));
        let accepted = Arc::new(Mutex::new(0usize));
        let rejected = Arc::new(Mutex::new(0usize));

        let a = Arc::clone(&accepted);
        dialog.accepted.connect(move || {
            if let Ok(mut n) = a.lock() {
                *n += 1;
            }
        });

        let r = Arc::clone(&rejected);
        dialog.rejected.connect(move || {
            if let Ok(mut n) = r.lock() {
                *n += 1;
            }
        });

        dialog.select_file("/tmp/a.txt");
        dialog.show();
        dialog.handle_event(&Event::key_press(13, 0));
        assert_eq!(*accepted.lock().expect("accepted lock"), 1);
        assert!(!dialog.is_visible());

        dialog.show();
        dialog.select_file("/tmp/b.txt");
        dialog.handle_event(&Event::key_press(27, 0));
        assert_eq!(*rejected.lock().expect("rejected lock"), 1);
        assert!(dialog.selected_files().is_empty());
        assert!(!dialog.is_visible());
    }
}
