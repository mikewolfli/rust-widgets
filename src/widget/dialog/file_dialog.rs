// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! File dialog widget.
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::tr;

use crate::impl_widget_property_hooks;
use crate::property_names_of;
use crate::widget::capability::coercion::{expect_bool, expect_string};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
/// File dialog mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileDialogMode {
    /// Choose a single existing file.
    OpenFile,
    /// Choose one or more existing files.
    OpenFiles,
    /// Choose a destination path, which need not exist yet.
    SaveFile,
    /// Choose a directory rather than a file.
    SelectDirectory,
}
/// File name filter entry.
#[derive(Debug, Clone)]
pub struct FileFilter {
    /// The human-readable filter name shown in the dialog's filter drop-down,
    /// e.g. `"Images"`.
    pub description: String,
    /// The extensions the filter accepts, written **without** the leading dot —
    /// `"png"`, not `".png"`. [Self's `Display`] is what adds the `*.` prefix, so
    /// storing a dotted value here produces `"*.png"`-style duplication in the
    /// rendered filter text. The single entry `"*"` means "all files".
    pub extensions: Vec<String>,
}
impl FileFilter {
    /// Builds a filter from its description and extension list.
    ///
    /// Nothing is validated or normalised: extensions are stored exactly as
    /// given, dots included, so the caller is responsible for the no-dot
    /// convention described on [`FileFilter::extensions`].
    pub fn new(description: impl Into<String>, extensions: Vec<impl Into<String>>) -> Self {
        Self {
            description: description.into(),
            extensions: extensions.into_iter().map(|e| e.into()).collect(),
        }
    }
    /// The catch-all filter: a translated "all files" description matching `*`.
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
    /// Emitted by [`FileDialog::accept`] with the full selection — every chosen
    /// path, not just the first. Not emitted when nothing is selected.
    pub files_selected: Signal1<Vec<String>>,
    /// Emitted by [`FileDialog::select_file`] with the newly chosen path.
    pub file_selected: Signal1<String>,
    /// Emitted when the file under the cursor changes. Nothing in this widget
    /// emits it — it has no file list of its own to move through — so it exists
    /// for a host that drives the selection.
    pub current_changed: Signal1<String>,
    /// Emitted by [`FileDialog::accept`], after `files_selected`.
    pub accepted: GenericSignal,
    /// Emitted by [`FileDialog::reject`].
    pub rejected: GenericSignal,
}
impl FileDialog {
    /// Creates an open-file dialog in `geometry`.
    ///
    /// Starts in [`FileDialogMode::OpenFile`], modal, with an empty directory and
    /// selection, and a single "all files" filter selected.
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
    /// Whether the dialog blocks interaction with its owner while open.
    ///
    /// This records the intent; enforcement is the modal stack in
    /// [`crate::widget::runtime`] (`enter_modal` / `exit_modal`). On by default.
    pub fn is_modal(&self) -> bool {
        self.modal
    }
    /// Sets the modality flag and repaints. See [`FileDialog::is_modal`].
    pub fn set_modal(&mut self, modal: bool) {
        self.modal = modal;
        self.base.request_redraw();
    }
    /// Creates a dialog configured to open a single existing file.
    ///
    /// Equivalent to [`FileDialog::new`] followed by setting the mode, which is
    /// also the title's source: the title is set to the translated "open file"
    /// string, so any directory or selection the caller had set is not preserved
    /// (there is none yet — this is a constructor).
    pub fn open_file(geometry: Rect) -> Self {
        let mut d = Self::new(geometry);
        d.mode = FileDialogMode::OpenFile;
        d.title = tr!("dialog.file_dialog.open_file");
        d
    }
    /// Creates a dialog configured to choose a save destination.
    ///
    /// Like [`FileDialog::open_file`], the title is set to the translated "save
    /// file" string.
    pub fn save_file(geometry: Rect) -> Self {
        let mut d = Self::new(geometry);
        d.mode = FileDialogMode::SaveFile;
        d.title = tr!("dialog.file_dialog.save_file");
        d
    }
    /// What the dialog is choosing.
    pub fn mode(&self) -> FileDialogMode {
        self.mode
    }
    /// The dialog's title, as shown in its header bar.
    pub fn title(&self) -> &str {
        &self.title
    }
    /// The directory the dialog is browsing, or `""` when none was set.
    ///
    /// Nothing in this widget populates it: it is a slot the host fills in, and
    /// no file listing is read from it here.
    pub fn directory(&self) -> &str {
        &self.directory
    }
    /// Every path currently selected, in selection order. Empty until
    /// [`FileDialog::select_file`] is called; this widget never populates it from
    /// a directory listing.
    pub fn selected_files(&self) -> &[String] {
        &self.selected_files
    }
    /// The first selected path, or `None` when the selection is empty.
    ///
    /// This is the convenience accessor for the single-selection modes; for
    /// [`FileDialogMode::OpenFiles`] read [`FileDialog::selected_files`], since
    /// this discards all but the first.
    pub fn selected_file(&self) -> Option<&str> {
        self.selected_files.first().map(|s| s.as_str())
    }
    /// The configured file name filters, in the order they are offered.
    pub fn name_filters(&self) -> &[FileFilter] {
        &self.name_filters
    }
    /// The filter currently in effect.
    ///
    /// Always the first filter after construction or
    /// [`FileDialog::set_name_filters`], because nothing here changes the
    /// selection. `None` only if the filter list was emptied.
    pub fn current_filter(&self) -> Option<&FileFilter> {
        self.name_filters.get(self.current_filter)
    }
    /// Switches the dialog to `mode` and repaints.
    ///
    /// The title is **overwritten** with the translated string for the new mode —
    /// open file for the two open modes, save file for
    /// [`FileDialogMode::SaveFile`], select directory for
    /// [`FileDialogMode::SelectDirectory`] — so a custom title must be reapplied
    /// afterwards with [`FileDialog::set_title`].
    pub fn set_mode(&mut self, mode: FileDialogMode) {
        self.mode = mode;
        self.title = tr!(match mode {
            FileDialogMode::OpenFile | FileDialogMode::OpenFiles => "dialog.file_dialog.open_file",
            FileDialogMode::SaveFile => "dialog.file_dialog.save_file",
            FileDialogMode::SelectDirectory => "dialog.file_dialog.select_directory",
        });
        self.base.request_redraw();
    }
    /// Sets the title shown in the header bar and repaints.
    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title = title.into();
        self.base.request_redraw();
    }
    /// Sets the directory the dialog reports as current and repaints.
    ///
    /// Purely informational — see [`FileDialog::directory`].
    pub fn set_directory(&mut self, dir: impl Into<String>) {
        self.directory = dir.into();
        self.base.request_redraw();
    }
    /// Replaces the filter list and repaints.
    ///
    /// Resets the current filter to the first one, so any previous selection is
    /// lost. Passing an empty list leaves [`FileDialog::current_filter`]
    /// returning `None`.
    pub fn set_name_filters(&mut self, filters: Vec<FileFilter>) {
        self.name_filters = filters;
        self.current_filter = 0;
        self.base.request_redraw();
    }
    /// Replaces the selection with a single path and emits `file_selected` with
    /// it.
    ///
    /// This is a **setter, not a toggle**: any previous selection is discarded,
    /// so it cannot be used to build up a multi-file selection for
    /// [`FileDialogMode::OpenFiles`]. No repaint is requested.
    pub fn select_file(&mut self, path: impl Into<String>) {
        let path = path.into();
        self.selected_files = vec![path.clone()];
        self.file_selected.emit(path);
    }
    /// Confirms the dialog: emits `files_selected` with the whole selection (or
    /// nothing at all when the selection is empty), then `accepted`, then hides
    /// the dialog.
    ///
    /// The selection is **not** cleared, so it can be read back after the dialog
    /// closes. Pressing Enter (key code 13) while the dialog is enabled and
    /// visible does the same.
    pub fn accept(&mut self) {
        if !self.selected_files.is_empty() {
            self.files_selected.emit(self.selected_files.clone());
        }
        self.accepted.emit();
        self.hide();
    }
    /// Cancels the dialog: clears the selection, emits `rejected`, and hides the
    /// dialog.
    ///
    /// Unlike [`FileDialog::accept`] the selection is discarded, so it cannot be
    /// read back afterwards. Pressing Escape (key code 27) has the same effect.
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

    /// Reports this widget as the object that paints it.
    ///
    /// `FileDialog` implements `Draw`, so `Some(self)` is total and cannot be
    /// wrong.
    fn as_draw_mut(&mut self) -> Option<&mut dyn crate::widget::Draw> {
        Some(self)
    }

    impl_widget_property_hooks!();
}

/// `FileDialog`'s property contract.
///
/// Read semantics are carried over unchanged from the centralised
/// `access_read_dialog.in.rs` dispatch. Both properties are read-only: the old
/// write layer had no arm for this kind, so a write answers
/// `UnknownProperty` through the shared fallback.
impl WidgetProperties for FileDialog {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "title" => Ok(CapabilityValue::String(self.title().to_string())),
            "modal" => Ok(CapabilityValue::Bool(self.is_modal())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "title" => {
                self.set_title(expect_string(value)?);
                Ok(())
            }
            "modal" => {
                self.set_modal(expect_bool(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["title", "modal", BASE_PROPERTY_NAMES]
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
