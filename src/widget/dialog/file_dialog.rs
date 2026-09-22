// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! File dialog widget.
use crate::core::{Color, Font, HorizontalAlignment, Rect, Size};
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
use crate::widget::metrics::{dimensions, ControlMetrics};
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
/// `FileDialog`'s list top gap below the title bar.
///
/// The strip between the title bar's bottom edge and the list's own top edge. Named once
/// so the layout stack and the frame's own padding agree; the draw path used to write this
/// as `list_y = rect.y + 38` — a bare offset that mixed the title-bar height with the gap
/// and then cancelled against a second literal in the height arithmetic.
const LIST_TOP_GAP: i32 = 10;
/// The selected-file strip's height: one 22 px row above the button row.
const SEL_H: i32 = 22;
/// The gap between the list and the selected-file strip below it.
const SELECTED_STRIP_GAP: i32 = 10;

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
    /// Emitted when the file under the cursor changes.
    ///
    /// The dialog has no file list of its own, so nothing here moves a cursor: a
    /// host that owns the list reports movement through
    /// [`FileDialog::set_current_file`], which emits this.
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
    /// Reports that the host's file cursor moved onto `path`, emitting
    /// `current_changed`.
    ///
    /// This dialog draws no file list, so the cursor is the host's state; this is
    /// the reporting path that makes the `current_changed` contract obtainable
    /// rather than an advertised signal with no emitter. It does **not** change the
    /// selection — see [`FileDialog::select_file`] for that.
    pub fn set_current_file(&mut self, path: impl Into<String>) {
        self.current_changed.emit(path.into());
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

    /// Runs one of the commands `file_dialog` publishes.
    ///
    /// `open` is the dialog's own accept action and maps onto the real `accept`;
    /// it takes no payload. `set_mode` and `set_directory` carry the value the
    /// caller wants and belong on the property/inherent route, so a bare
    /// invocation is reported as needing one rather than being called unknown.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "open" => {
                self.accept();
                Ok(())
            }
            "set_mode" | "set_directory" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
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
impl FileDialog {
    /// The frame the dialog actually paints: at most its intrinsic size, centred in the
    /// area it was given.
    ///
    /// # Why the frame is not the caller's rectangle
    ///
    /// `rect` is the area the dialog is **offered**. Painting it verbatim drew the 240x120
    /// census cell as a 240x120 frame whose list, selected-file strip and button row were
    /// then stacked against literals (`list_y = rect.y + 38`, `rect.height - btn_h - 12`)
    /// written for the 400 px default size — the list collapsed to a few pixels and the
    /// strip below it overlapped the buttons. [`ControlMetrics::painted_box`] caps each axis
    /// at the dialog's own intrinsic size and centres what is left; every band below is
    /// derived from this one rect, so the rows stack inside the frame instead of each being
    /// hung off an absolute offset.
    fn frame_rect(&self) -> Rect {
        ControlMetrics::painted_box(
            self.base.geometry(),
            Size::new(dimensions::DIALOG_MIN_WIDTH, dimensions::DIALOG_MIN_HEIGHT),
        )
    }
}

impl Draw for FileDialog {
    fn draw(&mut self, context: &mut RenderContext) {
        // The **frame**, not the control's rectangle: see `frame_rect`.
        let rect = self.frame_rect();
        let style = self.style().clone();

        // Chrome colours resolve explicit style first, then the theme's resolved style
        // for this control, and only then fall back to a literal. The style step alone
        // was not enough: `WidgetStyle` carries no title-bar or accent field, so the two
        // most visible pixels of the dialog — the title band and the accept button —
        // stayed a hardcoded blue in either appearance, and the render census reported
        // the whole control as theme-blind.
        //
        // The theme read is a separate manager lock, taken and released inside
        // `resolved_theme_style`, so it is not held across the draw — the global
        // manager's mutex is not re-entrant.
        let theme = crate::style::resolved_theme_style("file_dialog");
        // The window fill and the accent are read as their own lock acquisition and copied
        // out as values, so the guard is dropped before anything else touches the theme —
        // the global manager's mutex is not re-entrant.
        let (window_fill, accent) = {
            let manager = crate::style::theme_manager();
            match manager.current_theme() {
                Some(active) => (active.colors.background, active.colors.primary),
                None => (Color::WHITE, Color::rgb(0, 120, 215)),
            }
        };
        let accent_ink = accent.contrast_color();

        // A dialog is a `Surface`-role control and `Surface` resolves to the window's own
        // fill, which would leave the frame invisible against the window. A resolved
        // surface equal to the window fill is therefore re-derived one step toward the
        // ink, the same distinction `Colors::input_background` draws for a field.
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);
        let surface = match style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
        {
            Some(resolved) if resolved != window_fill => resolved,
            _ => window_fill.blend(&ink, 0.06),
        };
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or(Color::rgb(160, 160, 160));
        // The field interiors are one step *away* from the dialog surface, so they read as
        // editable regions. On a light surface that means darker and on a dark one lighter —
        // the old code forced white, which on a dark theme was a glaring rectangle.
        let field = if surface.is_dark() {
            surface.blend(&Color::WHITE, 0.08)
        } else {
            surface.blend(&Color::BLACK, 0.06)
        };

        // Rounded by [`dimensions::DIALOG_RADIUS`]; the radius is clamped to the frame so a
        // box smaller than its own corner is not drawn with an inverted one.
        let radius = dimensions::DIALOG_RADIUS.min(rect.width / 2).min(rect.height / 2);
        if radius > 0 {
            context.fill_rounded_rect(rect, radius, surface);
            context.draw_rounded_rect_stroke(rect, radius, border, 1);
        } else {
            context.fill_rect(rect, surface);
            context.draw_rect(rect, border);
        }
        // Title bar: a separate region from the dialog surface, in the theme's accent
        // rather than the literal blue it carried before, so the bar follows the palette
        // the rest of the application is using. The label is centred on the bar's own band
        // through the shared primitive and fitted to it, so a long title truncates at the
        // bar's edge instead of running past the frame. The strip comes from `top_band`.
        let title_bar_band = ControlMetrics::top_band(rect, dimensions::DIALOG_TITLE_BAR_HEIGHT);
        context.fill_rect(title_bar_band, accent);
        if !self.title.is_empty() {
            let title_font = Font::default();
            let title_line = context.text_line(title_bar_band, &title_font);
            context.draw_text_fitted(
                Rect::new(
                    rect.x + 8,
                    title_line.y,
                    rect.width.saturating_sub(16),
                    title_line.height.max(1),
                ),
                &self.title,
                &title_font,
                accent_ink,
                HorizontalAlignment::Left,
            );
        }
        // The three stacked rows are the frame's own bands: the button row at the bottom,
        // the selected-file strip above it, and the list taking the remainder. Deriving all
        // three from the frame is what keeps them from overlapping on a short dialog — each
        // was previously placed from an absolute offset written for a taller one.
        let button_band = ControlMetrics::bottom_band(rect, dimensions::DIALOG_BUTTON_HEIGHT);
        let button_top = button_band.y;
        // The list top is the title bar's bottom edge plus the frame's own padding.
        let list_y = rect.y + dimensions::DIALOG_TITLE_BAR_HEIGHT as i32 + LIST_TOP_GAP;
        // The list has to hold at least the placeholder's own line box, since that is the
        // only thing it draws when empty. The selected-file strip below it is the element
        // that yields when the two cannot both have room: a 6 px list under a 14 px line
        // would be the same defect in miniature, with the list's own explanation hanging
        // out of it. Only when even a line does not fit is the list clamped to what
        // remains, rather than reaching below the button row.
        let placeholder_line_h = context.measure_text("M", &Font::default()).height.max(1) as i32;
        let list_min_h = placeholder_line_h + 4;
        let reserved_below = SEL_H + SELECTED_STRIP_GAP;
        let list_h = if button_top - list_y >= list_min_h {
            // The list's own remainder once the strip below is reserved, floored at one
            // line so an empty list can always draw the text that explains it.
            (button_top - reserved_below - list_y).max(list_min_h) as u32
        } else {
            // Not even a line fits: the list takes what is left and the strip yields.
            (button_top - list_y).max(0) as u32
        };
        let list_rect = Rect::new(rect.x + 10, list_y, rect.width.saturating_sub(20), list_h);
        if list_rect.height > 0 {
            context.fill_rect(list_rect, field);
            context.draw_rect(list_rect, border);
        }
        // The placeholder lives inside the list it describes, so it truncates at the
        // list's edge rather than at a coordinate chosen for a wider default size. Its
        // line box is centred in the list via the shared primitive: the glyph origin is
        // the box's top-left, so the old fixed `list_rect.y + 6` placed a 14 px line
        // from y=44 to 58 inside a list ending at 46 — twelve pixels of the text that
        // explains the empty list were outside the well it belongs to. Drawn only when the
        // list is non-empty, so a squeezed dialog emits no `<text …></text>`.
        let placeholder = tr!("dialog.file_dialog.file_list_placeholder");
        let placeholder_font = Font::default();
        if list_rect.height > 0 && !placeholder.is_empty() {
            let placeholder_band = Rect::new(
                list_rect.x + 6,
                list_rect.y,
                list_rect.width.saturating_sub(12),
                list_rect.height,
            );
            let placeholder_line = context.text_line(placeholder_band, &placeholder_font);
            context.draw_text_fitted(
                Rect::new(
                    placeholder_band.x,
                    placeholder_line.y,
                    placeholder_band.width.max(1),
                    placeholder_line.height.max(1),
                ),
                &placeholder,
                &placeholder_font,
                // Dimmed toward the field, then held to the text floor: a fixed 50% blend
                // measured 3.65:1 on the light field, so the line explaining what the empty
                // list is for was itself hard to read.
                ink.blend(&field, 0.5).legible_on(field, 4.5),
                HorizontalAlignment::Left,
            );
        }
        // Selected files display: a 70 px label column followed by the file-name field.
        // The strip is placed above the button row rather than eight pixels below the
        // list, which is what used to let it collide with — and overlap — the buttons when
        // the list had been squeezed to nothing. It is drawn only when it genuinely fits
        // between the list's bottom and the button row: on a dialog too short for both,
        // the list keeps its line and the strip is dropped rather than being pushed over
        // the buttons by the `max` that used to guarantee it a place.
        let sel_h = SEL_H;
        let sel_y = button_top - sel_h - SELECTED_STRIP_GAP;
        let sel_fits = sel_y >= list_y + list_h as i32 + 4;
        if sel_fits {
            let sel_label = tr!("dialog.file_dialog.file_name");
            let sel_label_font = Font::default();
            let sel_band = Rect::new(rect.x + 10, sel_y, 66, sel_h as u32);
            let sel_line = context.text_line(sel_band, &sel_label_font);
            context.draw_text_fitted(
                Rect::new(sel_band.x, sel_line.y, sel_band.width, sel_line.height.max(1)),
                &sel_label,
                &sel_label_font,
                ink,
                HorizontalAlignment::Left,
            );
            let fname = self.selected_file().unwrap_or("");
            let fname_rect =
                Rect::new(rect.x + 80, sel_y, rect.width.saturating_sub(90), sel_h as u32);
            context.fill_rect(fname_rect, field);
            context.draw_rect(fname_rect, border);
            // Guarded: an unguarded draw of an empty file name emits `<text …></text>`.
            if !fname.is_empty() {
                let fname_font = Font::default();
                let fname_band = Rect::new(
                    fname_rect.x + 4,
                    fname_rect.y,
                    fname_rect.width.saturating_sub(8),
                    fname_rect.height,
                );
                let fname_line = context.text_line(fname_band, &fname_font);
                context.draw_text_fitted(
                    fname_line,
                    fname,
                    &fname_font,
                    ink,
                    HorizontalAlignment::Left,
                );
            }
        } else {
            // The file name still needs a home; it moves into the list's own band, which
            // is what the list is for. This keeps the control's content reachable at a
            // size where the strip has no room, rather than silently dropping it.
            let fname = self.selected_file().unwrap_or("");
            let fname_font = Font::default();
            let band = Rect::new(
                list_rect.x + 6,
                list_rect.y,
                list_rect.width.saturating_sub(12),
                list_rect.height,
            );
            if !fname.is_empty() && band.height > 0 {
                let line = context.text_line(band, &fname_font);
                context.draw_text_fitted(line, fname, &fname_font, ink, HorizontalAlignment::Left);
            }
        }
        // OK/Cancel buttons. The pair is right-aligned inside the frame through the shared
        // [`action_row_geometry`] derivation, so this dialog and the seven others that draw the
        // same pair cannot disagree about the button width, the gap between them or where the
        // row's left edge falls. The row is the frame's bottom band, and the labels are centred
        // in their buttons and fitted to them, so a truncating locale cannot spill out of the
        // button it belongs to.
        let ok_label = if self.mode == FileDialogMode::SaveFile {
            tr!("common.button.save")
        } else {
            tr!("common.button.open")
        };
        let labels = vec![ok_label, tr!("common.button.cancel")];
        let row = super::message_box::action_row_geometry(context, &labels, button_band, true);
        let font = Font::default();
        // The accept button is the dialog's call to action: the theme's accent, with its
        // contrast colour as the label — the same pairing `WidgetRole::Primary` uses.
        let ok_rect = row.buttons[0];
        context.fill_rect(ok_rect, accent);
        context.draw_text_line(ok_rect, &labels[0], &font, accent_ink, HorizontalAlignment::Center);
        let cancel_rect = row.buttons[1];
        context.fill_rect(cancel_rect, surface.blend(&ink, 0.1));
        context.draw_rect(cancel_rect, border);
        context.draw_text_line(cancel_rect, &labels[1], &font, ink, HorizontalAlignment::Center);
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

    #[test]
    fn set_current_file_emits_current_changed_without_touching_the_selection() {
        // `current_changed` was declared and documented but had no emitter, so a
        // subscriber could connect and never fire. This pins the reporting path.
        let mut dialog = FileDialog::new(Rect::new(0, 0, 420, 280));
        let seen = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = Arc::clone(&seen);

        dialog.current_changed.connect(move |path| {
            if let Ok(mut v) = sink.lock() {
                v.push((*path).clone());
            }
        });

        dialog.set_current_file("/tmp/hovered.txt");
        dialog.set_current_file("/tmp/other.txt");

        assert_eq!(
            *seen.lock().expect("seen lock"),
            vec!["/tmp/hovered.txt".to_string(), "/tmp/other.txt".to_string()]
        );
        // Moving the cursor is not a selection.
        assert_eq!(dialog.selected_file(), None);
    }
}
