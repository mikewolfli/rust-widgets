// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! File dialog widget — the crate's one file chooser, and it is **self-drawn**.
//!
//! # What this control is, and what the host owes it
//!
//! There is no second, host-provided chooser behind this widget: every backend draws this one
//! (see `PlatformCapabilities`, which has no `file_dialog` flag for exactly that reason). What the
//! control does **not** own is the directory listing — the host supplies the rows and reports the
//! user's movement through [`FileDialog::set_current_file`]. The library never browses a directory
//! on the host's behalf.
//!
//! The host's half is therefore one loop, and all of it goes through this widget's own API:
//!
//! * the user picks a row in the host's list → [`FileDialog::set_current_file`] (cursor) and the
//!   writable `selected_file` property (selection), which is why that property is *not* read-only;
//! * the user confirms → [`FileDialog::accept`], which emits `files_selected` with the whole
//!   selection;
//! * the user dismisses → [`FileDialog::reject`].
//!
//! A host that mounts the dialog and then only reads it has a box that can never be filled — which
//! is why the selection has a reachable write path from an `ObjectId`.
use crate::compat::{Rc, RefCell};
use crate::core::{Color, HorizontalAlignment, ObjectId, Point, Rect, Size};
use crate::event::key_codes;
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
use crate::widget::metrics::{dimensions, effective_font, ControlMetrics};
use crate::widget::{BaseWidget, Draw, IconName, IconRef, IconSource, Widget, WidgetKind};
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
/// One row in the host-supplied directory listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileEntry {
    /// The entry's name, without its directory prefix.
    pub name: String,
    /// Whether the entry is a directory.
    ///
    /// This is the navigation fact (a press opens it and the dialog descends into it). The
    /// details view's *type* column reads [`Self::kind`], which can say more than two things.
    pub is_directory: bool,
    /// The entry's size in bytes, when the host knows it.
    ///
    /// `None` is kept distinct from `Some(0)`: a directory, or a source that does not
    /// report sizes, has no size rather than a zero one. The details view sorts `None`
    /// after every reported size and paints an empty cell instead of inventing a number.
    pub size: Option<u64>,
    /// The entry's modification time as seconds since the Unix epoch, when known.
    pub modified: Option<u64>,
    /// What the entry is, as the details view's type column spells it.
    pub kind: FileKind,
    /// The icon the listing paints beside the name, when the host supplies one.
    pub icon: Option<IconSource>,
}

impl FileEntry {
    /// Builds a listing row with no size or modification time.
    ///
    /// The kind follows `is_directory`; use [`Self::with_kind`] to replace it with a finer
    /// classification (a symlink, an unknown host entry).
    pub fn new(name: impl Into<String>, is_directory: bool) -> Self {
        let kind = if is_directory { FileKind::Directory } else { FileKind::File };
        Self { name: name.into(), is_directory, size: None, modified: None, kind, icon: None }
    }

    /// Adds the metadata a details view paints, leaving the name and type unchanged.
    pub fn with_metadata(mut self, size: Option<u64>, modified: Option<u64>) -> Self {
        self.size = size;
        self.modified = modified;
        self
    }

    /// Replaces the entry's classification without changing its navigation flag.
    pub fn with_kind(mut self, kind: FileKind) -> Self {
        self.kind = kind;
        self
    }

    /// Attaches the icon painted beside the name.
    pub fn with_icon(mut self, icon: IconSource) -> Self {
        self.icon = Some(icon);
        self
    }
}

/// How a [`FileEntry`] is classified in the details view's type column.
///
/// Separate from [`FileEntry::is_directory`] because the type column is a *description* while the
/// flag is a navigation decision: a host can report a symlink to a directory without making the
/// dialog descend into it, and an unknown entry still needs a word in the column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FileKind {
    /// A directory.
    Directory,
    /// A regular file.
    File,
    /// A symbolic link.
    Symlink,
    /// The host did not classify the entry.
    #[default]
    Unknown,
}

impl FileKind {
    /// The stable property spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            FileKind::Directory => "directory",
            FileKind::File => "file",
            FileKind::Symlink => "symlink",
            FileKind::Unknown => "unknown",
        }
    }

    /// The word the type column paints.
    pub fn label(self) -> &'static str {
        match self {
            FileKind::Directory => "Folder",
            FileKind::File => "File",
            FileKind::Symlink => "Link",
            FileKind::Unknown => "Unknown",
        }
    }

    /// The type-column sort rank: directories first, then files, links and unknowns.
    fn sort_rank(self) -> u8 {
        match self {
            FileKind::Directory => 0,
            FileKind::File => 1,
            FileKind::Symlink => 2,
            FileKind::Unknown => 3,
        }
    }
}

/// How the listing is painted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    /// One name per row, the original view.
    List,
    /// Name, size, modification time and type columns.
    Details,
}

impl ViewMode {
    /// The stable property spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            ViewMode::List => "list",
            ViewMode::Details => "details",
        }
    }

    /// Parses the property spelling; `None` for an unrecognised token so a write is refused
    /// rather than silently choosing a view the caller did not ask for.
    pub fn from_token(token: &str) -> Option<Self> {
        match token {
            "list" => Some(ViewMode::List),
            "details" => Some(ViewMode::Details),
            _ => None,
        }
    }
}

/// The column `FileDialog::sort_by` orders the listing by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortColumn {
    /// The entry name.
    Name,
    /// The reported size; entries with no size sort last.
    Size,
    /// The reported modification time; entries with none sort last.
    Modified,
    /// The entry kind, directories first, then name.
    Type,
}

impl SortColumn {
    /// The stable property spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            SortColumn::Name => "name",
            SortColumn::Size => "size",
            SortColumn::Modified => "modified",
            SortColumn::Type => "type",
        }
    }
}

/// How a pointer press combines with the existing selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SelectionModifier {
    /// Replace the selection with the pressed row.
    #[default]
    Replace,
    /// Add the pressed row, or remove it when it was already selected.
    Ctrl,
    /// Select the inclusive range from the anchor to the pressed row.
    Shift,
}

impl SelectionModifier {
    /// Reads the modifier a press was made under.
    pub fn from_event_bits(bits: u32) -> Self {
        let modifiers = crate::shortcut::Modifiers::from_event_bits(bits);
        if modifiers.contains(crate::shortcut::Modifiers::CTRL) {
            SelectionModifier::Ctrl
        } else if modifiers.contains(crate::shortcut::Modifiers::SHIFT) {
            SelectionModifier::Shift
        } else {
            SelectionModifier::Replace
        }
    }
}

/// The host's directory listing.
///
/// The library never reads the filesystem: a host that owns a listing implements this and
/// installs it with [`FileDialog::set_source`]. The dialog then draws, scrolls and selects the
/// rows, and reports the user's choice through its existing signals.
pub trait FileEntrySource {
    /// The entries of `directory`, in display order.
    fn entries(&self, directory: &str) -> Vec<FileEntry>;

    /// Whether `path` exists. Used by the save-mode overwrite check.
    fn exists(&self, path: &str) -> bool {
        let _ = path;
        false
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

/// The facts [`FileDialog::visible_rows`] is a pure function of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct VisibleRowsKey {
    list: Rect,
    row_height: u32,
    header_height: u32,
    scroll: usize,
    entries: usize,
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
    /// Emitted by [`FileDialog::sort_by`] with the new column and whether the order is descending.
    ///
    /// The current sort is always ascending, so the flag is `false` today; it is carried so a host
    /// that grows a descending toggle does not need a second signal.
    pub sort_changed: Signal1<(SortColumn, bool)>,
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
    /// Emitted when a save would overwrite an existing path and the dialog is asking first.
    pub overwrite_requested: Signal1<String>,
    /// Emitted on a double-click of a non-directory row, so a host can open a preview.
    pub quick_look_requested: Signal1<String>,
    /// Emitted whenever the multi-row selection changes, with the full path list.
    pub selection_changed: Signal1<Vec<String>>,
    /// Emitted by [`FileDialog::navigate_to`] (and the history moves) with the directory
    /// that was entered.
    pub directory_entered: Signal1<String>,
    /// Emitted when a sidebar favourite is activated.
    pub url_selected: Signal1<String>,
    /// Emitted when Tab moves the focus ring; the payload is this dialog's own object id
    /// (the dialog is one `ObjectId`; its self-drawn buttons are not separate objects).
    pub focus_changed: Signal1<ObjectId>,
    /// The host's directory listing, if one was installed.
    source: Option<Box<dyn FileEntrySource>>,
    /// The listing last read from `source`.
    entries: Vec<FileEntry>,
    /// The selected row indices, in selection order.
    selected_rows: Vec<usize>,
    /// How the next pointer press combines with the selection.
    selection_modifier: SelectionModifier,
    /// How the listing is painted.
    view_mode: ViewMode,
    /// The action button the keyboard focus ring is on, indexing the action row.
    focus: Option<usize>,
    /// The first visible row.
    scroll: usize,
    /// A caller-supplied label for the file-name strip; empty uses the catalogue string.
    label_text: String,
    /// The extension appended to a save path that has none; empty disables it.
    default_suffix: String,
    /// Visited directories, oldest first.
    history: Vec<String>,
    /// The index into `history` of the current directory.
    history_index: usize,
    /// Favorite directories shown as a sidebar; empty hides the sidebar.
    favorites: Vec<String>,
    /// Whether a save asks before overwriting an existing path.
    overwrite_confirmation: bool,
    /// A save waiting on the overwrite answer.
    pending_overwrite: Option<String>,
    /// The file-name field's edit buffer (DLG-05). Empty until the user types or a selection
    /// seeds it; [`Self::name_editing`] says which of the two the paint should read.
    name_edit: String,
    /// Whether the file-name field is in edit mode.
    name_editing: bool,
    /// Whether the current sort column runs descending (DLG-07).
    sort_descending: bool,
    /// The column the current order was produced by, for the header's indicator (DLG-08).
    sort_column: Option<SortColumn>,
    /// Cached visible-row rectangles, keyed by [`VisibleRowsKey`] (G-P08).
    visible_cache: VisibleRowsCache,
    /// Cached measured line height, keyed by the resolved font size's bits (G-P07).
    ///
    /// `list_rect` and `list_row_height` are read by every event and frame. The measurement
    /// needs a paint backend, so it is taken once per font size and reused rather than built
    /// again per call.
    list_line_cache: core::cell::Cell<Option<(u32, u32)>>,
}

/// The cached visible-row rectangles and the key they were built for.
type VisibleRowsCache = RefCell<Option<(VisibleRowsKey, Rc<Vec<(usize, Rect)>>)>>;
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
            sort_changed: Signal1::new(),
            current_changed: Signal1::new(),
            accepted: GenericSignal::new(),
            rejected: GenericSignal::new(),
            overwrite_requested: Signal1::new(),
            quick_look_requested: Signal1::new(),
            selection_changed: Signal1::new(),
            directory_entered: Signal1::new(),
            url_selected: Signal1::new(),
            focus_changed: Signal1::new(),
            source: None,
            entries: Vec::new(),
            selected_rows: Vec::new(),
            selection_modifier: SelectionModifier::Replace,
            view_mode: ViewMode::List,
            focus: None,
            scroll: 0,
            label_text: String::new(),
            default_suffix: String::new(),
            history: Vec::new(),
            history_index: 0,
            favorites: Vec::new(),
            overwrite_confirmation: false,
            pending_overwrite: None,
            name_edit: String::new(),
            name_editing: false,
            sort_descending: false,
            sort_column: None,
            visible_cache: RefCell::new(None),
            list_line_cache: core::cell::Cell::new(None),
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
        if self.modal == modal {
            return;
        }
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
        let title = title.into();
        if self.title == title {
            return;
        }
        self.title = title;
        self.base.request_redraw();
    }
    /// Sets the directory the dialog reports as current and repaints.
    ///
    /// With a listing installed, this also re-reads it. It does not record history; use
    /// [`FileDialog::navigate_to`] for a user-driven move.
    pub fn set_directory(&mut self, dir: impl Into<String>) {
        let dir = dir.into();
        if self.directory == dir {
            return;
        }
        self.directory = dir;
        self.refresh_entries();
        self.base.request_redraw();
    }

    /// Installs the host's directory listing and reads the current directory.
    pub fn set_source(&mut self, source: Box<dyn FileEntrySource>) {
        self.source = Some(source);
        self.refresh_entries();
        self.base.request_redraw();
    }

    /// The rows last read from the host's listing.
    pub fn entries(&self) -> &[FileEntry] {
        &self.entries
    }

    /// The selected row indices, in selection order.
    pub fn selected_rows(&self) -> &[usize] {
        &self.selected_rows
    }

    /// The selected row, if one is selected.
    ///
    /// The cursor row for the single-selection modes. For
    /// [`FileDialogMode::OpenFiles`] read [`FileDialog::selected_rows`] /
    /// [`FileDialog::selected_files`], which keep every chosen row.
    pub fn selected_row(&self) -> Option<usize> {
        self.selected_rows.first().copied()
    }

    /// How the listing is painted.
    pub fn view_mode(&self) -> ViewMode {
        self.view_mode
    }

    /// Switches the listing between names only and the details columns.
    pub fn set_view_mode(&mut self, mode: ViewMode) {
        if self.view_mode == mode {
            return;
        }
        self.view_mode = mode;
        self.base.request_redraw();
    }

    /// The file-name field's current text: the edit buffer while typing, otherwise the
    /// selected file's name (DLG-05).
    fn display_name(&self) -> &str {
        if self.name_editing {
            &self.name_edit
        } else {
            self.selected_file().unwrap_or("")
        }
    }

    /// The modifier the next host-driven press applies. The event path derives it from the
    /// press itself; this setter exists for a host that synthesises a row selection.
    pub fn set_selection_modifier(&mut self, modifier: SelectionModifier) {
        self.selection_modifier = modifier;
    }

    /// The first visible row.
    pub fn scroll_offset(&self) -> usize {
        self.scroll
    }

    /// Orders the listing by `column` and repaints.
    ///
    /// Directories sort before files for every column so the two groups stay together no
    /// matter which field the caller asked for; within a group the ordering is ascending.
    pub fn sort_by(&mut self, column: SortColumn) {
        // Row indices address the pre-sort vector, so the selected paths are the stable
        // identity to re-resolve after the order changes.
        let selected_paths = self.selected_files.clone();
        self.sort_column = Some(column);
        let descending = self.sort_descending;
        self.entries.sort_by(|a, b| {
            let by_directory = b.is_directory.cmp(&a.is_directory);
            if by_directory != core::cmp::Ordering::Equal {
                return by_directory;
            }
            let order = match column {
                SortColumn::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                // `None` sorts after every reported value rather than comparing equal to 0.
                SortColumn::Size => a.size.unwrap_or(u64::MAX).cmp(&b.size.unwrap_or(u64::MAX)),
                SortColumn::Modified => {
                    a.modified.unwrap_or(u64::MAX).cmp(&b.modified.unwrap_or(u64::MAX))
                }
                SortColumn::Type => a
                    .kind
                    .sort_rank()
                    .cmp(&b.kind.sort_rank())
                    .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())),
            };
            if descending {
                order.reverse()
            } else {
                order
            }
        });
        self.selected_rows =
            selected_paths.iter().filter_map(|path| self.row_for_path(path)).collect();
        self.sort_changed.emit((column, self.sort_descending));
        *self.visible_cache.borrow_mut() = None;
        self.base.request_redraw();
    }

    /// Whether the listing orders the current column descending (DLG-07).
    pub fn sort_direction_descending(&self) -> bool {
        self.sort_descending
    }

    /// Sets the sort direction and re-applies the current column (DLG-07).
    ///
    /// Directories still sort before files; only the within-group order reverses. With no
    /// column sorted yet this records the direction for the next [`Self::sort_by`].
    pub fn set_sort_direction(&mut self, descending: bool) {
        if self.sort_descending == descending {
            return;
        }
        self.sort_descending = descending;
        if let Some(column) = self.sort_column {
            self.sort_by(column);
        } else {
            self.base.request_redraw();
        }
    }

    /// Re-reads the listing and resets the selection and scroll.
    fn refresh_entries(&mut self) {
        self.entries = match self.source.as_ref() {
            Some(source) => source.entries(&self.directory),
            None => Vec::new(),
        };
        self.selected_rows.clear();
        self.scroll = 0;
    }

    /// The label painted beside the file-name strip, or `""` when the catalogue string is used.
    pub fn label_text(&self) -> &str {
        &self.label_text
    }

    /// Replaces the file-name strip's label and repaints when it changes.
    pub fn set_label_text(&mut self, label: impl Into<String>) {
        let label = label.into();
        if self.label_text == label {
            return;
        }
        self.label_text = label;
        self.base.request_redraw();
    }

    /// The extension appended to a save path that has none.
    pub fn default_suffix(&self) -> &str {
        &self.default_suffix
    }

    /// Sets the save-path suffix and repaints when it changes.
    pub fn set_default_suffix(&mut self, suffix: impl Into<String>) {
        let suffix = suffix.into().trim_start_matches('.').to_string();
        if self.default_suffix == suffix {
            return;
        }
        self.default_suffix = suffix;
        self.base.request_redraw();
    }

    /// Whether a save asks before overwriting an existing path.
    pub fn overwrite_confirmation(&self) -> bool {
        self.overwrite_confirmation
    }

    /// Enables or disables the save-mode overwrite question.
    pub fn set_overwrite_confirmation(&mut self, enabled: bool) {
        if self.overwrite_confirmation == enabled {
            return;
        }
        self.overwrite_confirmation = enabled;
        self.base.request_redraw();
    }

    /// The sidebar's favorite directories.
    pub fn sidebar(&self) -> &[String] {
        &self.favorites
    }

    /// Replaces the sidebar and repaints when it changes. An empty list hides the sidebar.
    pub fn set_sidebar(&mut self, favorites: Vec<String>) {
        if self.favorites == favorites {
            return;
        }
        self.favorites = favorites;
        self.base.request_redraw();
    }

    /// Replaces the visited-directory list.
    ///
    /// The write path that makes a session's recent directories restorable: the list is
    /// replaced and the history cursor moves to the most recent entry (the last one). The
    /// current directory is not changed, so a caller that also wants to return there calls
    /// [`FileDialog::go_back`] or [`FileDialog::set_directory`] afterwards.
    pub fn set_history(&mut self, history: Vec<String>) {
        if self.history == history {
            return;
        }
        self.history = history;
        self.history_index = self.history.len().saturating_sub(1);
        self.base.request_redraw();
    }

    /// The visited directories, oldest first.
    pub fn history(&self) -> &[String] {
        &self.history
    }

    /// Whether a previous directory is available.
    pub fn can_go_back(&self) -> bool {
        self.history_index > 0
    }

    /// Whether a forward directory is available.
    pub fn can_go_forward(&self) -> bool {
        self.history_index + 1 < self.history.len()
    }

    /// Moves to `directory` and records it in the history.
    pub fn navigate_to(&mut self, directory: impl Into<String>) {
        let directory = directory.into();
        if directory == self.directory {
            return;
        }
        // The history is the list of *current* directories, so the entry the dialog is leaving
        // is pushed once and the new one is appended after it. Without the second push the
        // current directory was never in the list and "forward" had nothing to return to.
        if self.history.is_empty() {
            self.history.push(self.directory.clone());
            self.history_index = 0;
        }
        self.history.truncate(self.history_index + 1);
        self.history.push(directory.clone());
        self.history_index = self.history.len() - 1;
        self.directory = directory.clone();
        self.refresh_entries();
        self.directory_entered.emit(directory);
        self.base.request_redraw();
    }

    /// Moves to the previous directory in the history.
    pub fn go_back(&mut self) {
        if !self.can_go_back() {
            return;
        }
        self.history_index -= 1;
        self.directory = self.history[self.history_index].clone();
        self.refresh_entries();
        self.directory_entered.emit(self.directory.clone());
        self.base.request_redraw();
    }

    /// Moves to the next directory in the history.
    pub fn go_forward(&mut self) {
        if !self.can_go_forward() {
            return;
        }
        self.history_index += 1;
        self.directory = self.history[self.history_index].clone();
        self.refresh_entries();
        self.directory_entered.emit(self.directory.clone());
        self.base.request_redraw();
    }

    /// Moves to the parent of the current directory through the normal history path (DLG-06).
    ///
    /// Returns whether the dialog moved. The root has no parent, so it reports `false` rather
    /// than navigating to itself or inventing an empty path.
    pub fn go_to_parent(&mut self) -> bool {
        let trimmed = self.directory.trim_end_matches(['/', '\\']);
        if trimmed.is_empty() {
            return false;
        }
        let Some(position) = trimmed.rfind(['/', '\\']) else {
            return false;
        };
        let parent =
            if position == 0 { trimmed[..1].to_string() } else { trimmed[..position].to_string() };
        if parent == self.directory {
            return false;
        }
        self.navigate_to(parent);
        true
    }

    /// Replaces the selection with `paths` and announces it once.
    ///
    /// This is the multi-selection counterpart of [`FileDialog::select_file`]: rows are
    /// resolved against the current listing by name so [`FileDialog::selected_rows`] stays
    /// consistent, and `selection_changed` carries the full list.
    pub fn select_files(&mut self, paths: Vec<String>) {
        self.selected_files = paths;
        self.selected_rows =
            self.selected_files.iter().filter_map(|path| self.row_for_path(path)).collect();
        self.selection_changed.emit(self.selected_files.clone());
        self.base.request_redraw();
    }

    /// Adds `path` to the selection, or removes it when it is already selected.
    ///
    /// The append path [`FileDialogMode::OpenFiles`] needs: [`FileDialog::select_file`] is a
    /// setter, so it can never build a multi-row selection. Returns whether the selection
    /// changed; `selection_changed` is emitted only when it did.
    pub fn add_selected_file(&mut self, path: impl Into<String>) -> bool {
        let path = path.into();
        if let Some(position) = self.selected_files.iter().position(|existing| *existing == path) {
            self.selected_files.remove(position);
            if let Some(row) = self.row_for_path(&path) {
                self.selected_rows.retain(|selected| *selected != row);
            }
            self.selection_changed.emit(self.selected_files.clone());
            self.base.request_redraw();
            return true;
        }
        self.selected_files.push(path.clone());
        if let Some(row) = self.row_for_path(&path) {
            if !self.selected_rows.contains(&row) {
                self.selected_rows.push(row);
            }
        }
        self.selection_changed.emit(self.selected_files.clone());
        self.base.request_redraw();
        true
    }

    /// The listing row whose name addresses `path`.
    ///
    /// A host may hand back either a bare name or a full path, so both spellings are
    /// accepted; anything that addresses no row simply has no index.
    fn row_for_path(&self, path: &str) -> Option<usize> {
        self.entries.iter().position(|entry| {
            entry.name == path || path.rsplit(['/', '\\']).next() == Some(entry.name.as_str())
        })
    }

    /// Confirms a pending overwrite and completes the save.
    pub fn confirm_overwrite(&mut self) {
        if self.pending_overwrite.take().is_some() {
            self.accept_impl(false);
        }
    }

    /// Cancels a pending overwrite, leaving the dialog open.
    pub fn cancel_overwrite(&mut self) {
        if self.pending_overwrite.take().is_some() {
            self.base.request_redraw();
        }
    }

    /// The path a pending overwrite question is about.
    pub fn pending_overwrite(&self) -> Option<&str> {
        self.pending_overwrite.as_deref()
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
        self.selected_rows = self.row_for_path(&path).into_iter().collect();
        self.selected_files = vec![path.clone()];
        self.file_selected.emit(path);
        self.selection_changed.emit(self.selected_files.clone());
    }
    /// Clears the pending selection without closing the dialog.
    ///
    /// The counterpart to [`FileDialog::select_file`]: that one replaces the selection, this one
    /// removes it. Distinct from [`FileDialog::reject`], which also hides the dialog — a host whose
    /// own list the user deselected wants the box cleared, not dismissed.
    ///
    /// Repaints when there was something to clear, because the selected-file strip is drawn from
    /// this field.
    pub fn clear_selection(&mut self) {
        if self.selected_files.is_empty() && self.selected_rows.is_empty() {
            return;
        }
        self.selected_files.clear();
        self.selected_rows.clear();
        self.selection_changed.emit(Vec::new());
        self.base.request_redraw();
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
    /// In [`FileDialogMode::SaveFile`] a configured default suffix is appended to a path that
    /// has none, and — when [`FileDialog::overwrite_confirmation`] is on — an existing path
    /// asks first by emitting `overwrite_requested` instead of accepting.
    ///
    /// The selection is **not** cleared, so it can be read back after the dialog
    /// closes. Pressing Enter (key code 13) while the dialog is enabled and
    /// visible does the same.
    pub fn accept(&mut self) {
        self.accept_impl(true);
    }

    /// The shared accept path; `check_overwrite` is false only for the confirmed retry.
    fn accept_impl(&mut self, check_overwrite: bool) {
        // A typed name is the save destination: the field is the control the user just used,
        // so it wins over a stale row selection (DLG-05).
        if self.mode == FileDialogMode::SaveFile && !self.name_edit.is_empty() {
            self.selected_files = vec![self.name_edit.clone()];
            self.selected_rows = self.row_for_path(&self.name_edit).into_iter().collect();
        }
        if self.mode == FileDialogMode::SaveFile && !self.default_suffix.is_empty() {
            if let Some(path) = self.selected_files.first_mut() {
                let last = path.rsplit(['/', '\\']).next().unwrap_or(path.as_str());
                if !last.contains('.') {
                    path.push('.');
                    path.push_str(&self.default_suffix);
                }
            }
        }
        if check_overwrite && self.overwrite_confirmation {
            if let Some(path) = self.selected_files.first().cloned() {
                let exists = self.source.as_ref().is_some_and(|source| source.exists(&path));
                if exists {
                    self.pending_overwrite = Some(path.clone());
                    self.overwrite_requested.emit(path);
                    self.base.request_redraw();
                    return;
                }
            }
        }
        self.pending_overwrite = None;
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
        let had_selection = !self.selected_files.is_empty() || !self.selected_rows.is_empty();
        self.selected_files.clear();
        self.selected_rows.clear();
        if had_selection {
            self.selection_changed.emit(Vec::new());
        }
        self.rejected.emit();
        self.hide();
    }
}
impl Widget for FileDialog {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
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
    /// Resolves a published event name to the control's own signal.
    ///
    /// # Why this is explicit
    ///
    /// `connect_event` validates a name against the capability table and registers a hub slot; only
    /// `event_signal_dyn` joins that name to the signal the control actually **emits**. Without an
    /// arm a published name is valid and inert, which is the silent failure
    /// `tools/check_event_signal_dyn.sh` exists to make impossible: the arm set is compared against
    /// the capability's published names, so the two cannot drift.
    ///
    /// # Payload mapping
    ///
    /// A hub name carries no value, so a `Signal1<T>` needs a conversion. The one used here is the
    /// same spelling the property side uses, so a designer reads one representation per kind: a
    /// number as `Int`, a float as `Float`, a flag as `Bool`, text as `String`, and a structured
    /// payload as its debug spelling. A `None` maps to `Null`, which is the same value an absent
    /// optional property uses — so "no selection" is distinguishable from "selection 0".
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "file_selected" => {
                Some(EventSignalRef::mapped("file_selected", &self.file_selected, |v| {
                    CapabilityValue::String(v.clone())
                }))
            }
            "files_selected" => {
                Some(EventSignalRef::mapped("files_selected", &self.files_selected, |v| {
                    let mut items = crate::compat::Vec::with_capacity(v.len());
                    for path in v.iter() {
                        items.push(CapabilityValue::String(path.clone()));
                    }
                    CapabilityValue::Tuple(items)
                }))
            }
            "accepted" => Some(EventSignalRef::unit("accepted", &self.accepted)),
            "rejected" => Some(EventSignalRef::unit("rejected", &self.rejected)),
            "current_changed" => {
                Some(EventSignalRef::mapped("current_changed", &self.current_changed, |v| {
                    CapabilityValue::String(v.clone())
                }))
            }
            "overwrite_requested" => Some(EventSignalRef::mapped(
                "overwrite_requested",
                &self.overwrite_requested,
                |v| crate::widget::capability::CapabilityValue::String(v.clone()),
            )),
            "quick_look_requested" => Some(EventSignalRef::mapped(
                "quick_look_requested",
                &self.quick_look_requested,
                |v| crate::widget::capability::CapabilityValue::String(v.clone()),
            )),
            "selection_changed" => {
                Some(EventSignalRef::mapped("selection_changed", &self.selection_changed, |v| {
                    let mut items = crate::compat::Vec::with_capacity(v.len());
                    for path in v.iter() {
                        items.push(CapabilityValue::String(path.clone()));
                    }
                    CapabilityValue::Tuple(items)
                }))
            }
            "directory_entered" => {
                Some(EventSignalRef::mapped("directory_entered", &self.directory_entered, |v| {
                    CapabilityValue::String(v.clone())
                }))
            }
            "url_selected" => {
                Some(EventSignalRef::mapped("url_selected", &self.url_selected, |v| {
                    CapabilityValue::String(v.clone())
                }))
            }
            "focus_changed" => {
                Some(EventSignalRef::mapped("focus_changed", &self.focus_changed, |v| {
                    CapabilityValue::UInt(*v)
                }))
            }
            "sort_changed" => {
                Some(EventSignalRef::mapped("sort_changed", &self.sort_changed, |v| {
                    CapabilityValue::Tuple(crate::compat::Vec::from([
                        CapabilityValue::String(v.0.as_str().to_string()),
                        CapabilityValue::Bool(v.1),
                    ]))
                }))
            }
            _ => None,
        }
    }
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
            "mode" => Ok(CapabilityValue::String(file_dialog_mode_token(self.mode()).into())),
            "directory" => Ok(CapabilityValue::String(self.directory().to_string())),
            "selected_file" => Ok(match self.selected_file() {
                // No selection is `Null`, exactly the schema default and the value the
                // setter accepts to clear: an empty-name `""` would be a third spelling
                // of "nothing selected", and the setter's `Null` arm could never be read
                // back. (`tree_view`'s optional node properties answer the same way.)
                Some(path) => CapabilityValue::String(path.to_string()),
                None => CapabilityValue::Null,
            }),
            "label_text" => Ok(CapabilityValue::String(self.label_text().to_string())),
            "default_suffix" => Ok(CapabilityValue::String(self.default_suffix().to_string())),
            "file_count" => Ok(CapabilityValue::UInt(self.entries.len() as u64)),
            "can_go_back" => Ok(CapabilityValue::Bool(self.can_go_back())),
            "can_go_forward" => Ok(CapabilityValue::Bool(self.can_go_forward())),
            "view_mode" => Ok(CapabilityValue::String(self.view_mode.as_str().to_string())),
            "sidebar_urls" => Ok(CapabilityValue::Tuple(
                self.favorites
                    .iter()
                    .map(|favorite| CapabilityValue::String(favorite.clone()))
                    .collect(),
            )),
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
            // `mode` and `directory` are the two payloads `set_mode` / `set_directory` carry, so
            // they must be writable through the same route the command's documentation points at.
            // Declaring them read-only while publishing the commands left the payload with nowhere
            // to land — the schema/contract disagreement `capability::properties_tests` catches.
            "mode" => {
                let token = expect_string(value)?;
                let mode =
                    file_dialog_mode_from_token(&token).ok_or(CapabilityAccessError::OutOfRange)?;
                self.set_mode(mode);
                Ok(())
            }
            "directory" => {
                self.set_directory(expect_string(value)?);
                Ok(())
            }
            // `selected_file` is the dialog's *output* — but the host owns the list that produces
            // it, so the write path has to exist.
            //
            // # Why this used to be read-only, and why that was wrong
            //
            // The dialog draws no file list of its own: the host does (see
            // [`FileDialog::set_current_file`]). Setting the selection therefore used to be
            // `&mut FileDialog`-only, which works *before* `runtime::register` and nowhere after
            // it — once mounted, a host holds an `ObjectId`, and no downcast to `FileDialog`
            // exists. So a host whose own list the user had just clicked on could not hand the
            // chosen row to the box it had created: the "the list belongs to the host" design was
            // one-way in practice. Property writes are reachable by id, so this is the route that
            // closes it.
            //
            // `Null` clears the pending selection without closing the dialog, which is a fact a
            // `String` write cannot express.
            "selected_file" => match value {
                CapabilityValue::Null => {
                    self.clear_selection();
                    Ok(())
                }
                other => {
                    self.select_file(expect_string(other)?);
                    // `select_file` alone does not repaint (its doc says so), but a mounted dialog
                    // is already on screen, so a host's write must be visible without waiting for
                    // an unrelated event.
                    self.base.request_redraw();
                    Ok(())
                }
            },
            "label_text" => {
                self.set_label_text(expect_string(value)?);
                Ok(())
            }
            "default_suffix" => {
                self.set_default_suffix(expect_string(value)?);
                Ok(())
            }
            "view_mode" => {
                let token = expect_string(value)?;
                let mode = ViewMode::from_token(&token).ok_or(CapabilityAccessError::OutOfRange)?;
                self.set_view_mode(mode);
                Ok(())
            }
            // The sidebar is the persisted recent-URL list: a session can write it back,
            // and the read arm returns the same `Tuple` of strings so the round trip is
            // lossless.
            "sidebar_urls" => {
                let urls = crate::widget::capability::coercion::expect_string_list(value)?;
                self.set_sidebar(urls);
                Ok(())
            }
            "file_count" | "can_go_back" | "can_go_forward" => {
                Err(CapabilityAccessError::ReadOnlyProperty)
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "title",
            "modal",
            "mode",
            "directory",
            "selected_file",
            "label_text",
            "default_suffix",
            "file_count",
            "can_go_back",
            "can_go_forward",
            "view_mode",
            "sidebar_urls",
            BASE_PROPERTY_NAMES
        ]
    }

    fn property_tokens(&self, name: &str) -> &'static [&'static str] {
        match name {
            "mode" => FILE_DIALOG_MODE_TOKENS,
            "view_mode" => FILE_DIALOG_VIEW_MODE_TOKENS,
            _ => &[],
        }
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
        match event {
            Event::MousePress { pos, button: 1, modifiers } => {
                // A press's own Ctrl/Shift wins; a plain press leaves a modifier a host set
                // through `set_selection_modifier` in place for the row interaction that
                // follows. The row handler consumes it and restores `Replace`.
                let pressed = SelectionModifier::from_event_bits(*modifiers);
                if pressed != SelectionModifier::Replace {
                    self.selection_modifier = pressed;
                }
                if let Some(index) = self.button_at(*pos) {
                    self.focus = Some(index);
                    if index == 0 {
                        self.accept();
                    } else {
                        self.reject();
                    }
                    return;
                }
                if let Some(index) = self.sidebar_row_at(*pos) {
                    if let Some(favorite) = self.favorites.get(index).cloned() {
                        self.url_selected.emit(favorite.clone());
                        self.navigate_to(favorite);
                    }
                    return;
                }
                // A Details header is a sort control: pressing a column sorts by it, and
                // pressing the active column flips the direction (DLG-08).
                if self.view_mode == ViewMode::Details {
                    let list_rect = self.list_rect();
                    let header_h = self.list_header_height();
                    let header = Rect::new(list_rect.x, list_rect.y, list_rect.width, header_h);
                    if header_h > 0 && header.contains_point(*pos) {
                        if let Some((name, size, modified, kind)) = Self::detail_columns(header) {
                            let column = if name.contains_point(*pos) {
                                Some(SortColumn::Name)
                            } else if size.contains_point(*pos) {
                                Some(SortColumn::Size)
                            } else if modified.contains_point(*pos) {
                                Some(SortColumn::Modified)
                            } else if kind.contains_point(*pos) {
                                Some(SortColumn::Type)
                            } else {
                                None
                            };
                            if let Some(column) = column {
                                if self.sort_column == Some(column) {
                                    self.set_sort_direction(!self.sort_descending);
                                } else {
                                    self.sort_descending = false;
                                    self.sort_by(column);
                                }
                                return;
                            }
                        }
                    }
                }
                if self.list_rect().contains_point(*pos) {
                    self.press_list_row(*pos, false);
                }
            }
            Event::MouseDoubleClick { pos, button: 1 } => {
                if self.list_rect().contains_point(*pos) {
                    self.press_list_row(*pos, true);
                }
            }
            Event::Wheel { delta, .. } => {
                if self.entries.is_empty() || self.list_rect().height == 0 {
                    return;
                }
                let row_h = self.list_row_height().max(1);
                let header = self.list_header_height();
                let visible = (self.list_rect().height.saturating_sub(header) / row_h) as usize;
                let max_scroll = self.entries.len().saturating_sub(visible.max(1));
                let step = delta.y.unsigned_abs() as usize * 2;
                let next = if delta.y > 0 {
                    (self.scroll + step).min(max_scroll)
                } else {
                    self.scroll.saturating_sub(step)
                };
                if next != self.scroll {
                    self.scroll = next;
                    self.base.request_redraw();
                }
            }
            Event::TextInput { text } | Event::ImeCommit { text } => {
                // The file-name field accepts committed text; the desktop backends deliver
                // printable characters and IME commits here, not as key codes (DLG-05).
                if !self.name_editing {
                    self.name_editing = true;
                    self.name_edit = self.selected_file().unwrap_or("").to_string();
                }
                self.name_edit.extend(text.chars().filter(|ch| !ch.is_control()));
                self.base.request_redraw();
            }
            Event::KeyPress { key, .. } => {
                if *key == key_codes::ENTER {
                    self.accept();
                } else if *key == key_codes::ESCAPE {
                    self.reject();
                } else if *key == key_codes::TAB {
                    self.move_focus();
                } else if *key == key_codes::BACKSPACE || *key == key_codes::DELETE {
                    // Backspace removes the last character of the field; Delete does the same
                    // (there is no caret in this single-line field yet).
                    if !self.name_editing {
                        self.name_editing = true;
                        self.name_edit = self.selected_file().unwrap_or("").to_string();
                    }
                    self.name_edit.pop();
                    self.base.request_redraw();
                } else if *key == key_codes::UP {
                    // With no selection the first Up lands on the last row, so keyboard
                    // navigation works on a freshly opened dialog (DLG-09).
                    let target = self
                        .selected_row()
                        .map(|row| row.saturating_sub(1))
                        .unwrap_or_else(|| self.entries.len().saturating_sub(1));
                    self.keyboard_select(target);
                } else if *key == key_codes::DOWN {
                    let target = self
                        .selected_row()
                        .map(|row| (row + 1).min(self.entries.len().saturating_sub(1)))
                        .unwrap_or(0);
                    self.keyboard_select(target);
                } else if *key == key_codes::HOME {
                    self.keyboard_select(0);
                } else if *key == key_codes::END {
                    self.keyboard_select(self.entries.len().saturating_sub(1));
                } else if *key == key_codes::PAGE_UP || *key == key_codes::PAGE_DOWN {
                    let page = self.visible_row_count().max(1);
                    let current = self.selected_row().unwrap_or(0);
                    let target = if *key == key_codes::PAGE_UP {
                        current.saturating_sub(page)
                    } else {
                        (current + page).min(self.entries.len().saturating_sub(1))
                    };
                    self.keyboard_select(target);
                }
            }
            _ => { /* Other events are not relevant */ }
        }
    }
}

/// The accepted spellings of `FileDialog`'s `mode` property, in the order a chooser should
/// present them.
///
/// The schema's own list carried `["text", "integer", "double", "item"]` — the tokens of a
/// *different* dialog (an input dialog's value type), which no file dialog ever accepted. The list
/// here is the real one, and `capability::properties_tests` writes every token back through `set`,
/// so it cannot drift from the control's parser.
const FILE_DIALOG_MODE_TOKENS: &[&str] =
    &["open_file", "open_files", "save_file", "select_directory"];

/// The accepted spellings of the `view_mode` property.
const FILE_DIALOG_VIEW_MODE_TOKENS: &[&str] = &["list", "details"];

/// The token for a [`FileDialogMode`], the inverse of [`file_dialog_mode_from_token`].
fn file_dialog_mode_token(mode: FileDialogMode) -> &'static str {
    match mode {
        FileDialogMode::OpenFile => "open_file",
        FileDialogMode::OpenFiles => "open_files",
        FileDialogMode::SaveFile => "save_file",
        FileDialogMode::SelectDirectory => "select_directory",
    }
}

/// Parses a `mode` token; `None` for an unrecognised spelling, so `set_mode` refuses rather than
/// defaulting to a mode the caller did not ask for.
fn file_dialog_mode_from_token(token: &str) -> Option<FileDialogMode> {
    match token {
        "open_file" => Some(FileDialogMode::OpenFile),
        "open_files" => Some(FileDialogMode::OpenFiles),
        "save_file" => Some(FileDialogMode::SaveFile),
        "select_directory" => Some(FileDialogMode::SelectDirectory),
        _ => None,
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

    /// The sidebar's width: zero when no favorites were supplied.
    fn sidebar_width(&self) -> u32 {
        if self.favorites.is_empty() {
            0
        } else {
            96.min(self.base.geometry().width / 3)
        }
    }

    /// The list well's rectangle, derived from the same bands `draw` paints.
    fn list_rect(&self) -> Rect {
        let rect = self.frame_rect();
        let button_top = super::message_box::inset_action_band(
            ControlMetrics::bottom_band(rect, dimensions::DIALOG_BUTTON_HEIGHT),
            rect.y,
        )
        .y;
        let list_y = rect.y + dimensions::DIALOG_TITLE_BAR_HEIGHT as i32 + LIST_TOP_GAP;
        let sidebar = self.sidebar_width();
        let x = rect.x + 10 + sidebar as i32;
        let width = rect.width.saturating_sub(20 + sidebar);
        let line_h = self.list_line_height() as i32;
        let list_min_h = line_h + 4;
        let reserved_below = SEL_H + SELECTED_STRIP_GAP;
        let list_h = if button_top - list_y >= list_min_h {
            (button_top - reserved_below - list_y).max(list_min_h) as u32
        } else {
            (button_top - list_y).max(0) as u32
        };
        Rect::new(x, list_y, width, list_h)
    }

    /// The height of one listing row.
    fn list_row_height(&self) -> u32 {
        (self.list_line_height() + 6).max(18)
    }

    /// The measured line height, taken once per resolved font size (G-P07).
    fn list_line_height(&self) -> u32 {
        let font = effective_font(self.base.style());
        let key = font.size().to_bits();
        if let Some((cached_key, height)) = self.list_line_cache.get() {
            if cached_key == key {
                return height;
            }
        }
        let size = Size::new(self.base.geometry().width.max(1), self.base.geometry().height.max(1));
        let mut backend = crate::render::SoftwarePaintBackend::new(size, 1.0);
        let context = RenderContext::new(&mut backend);
        let height = context.measure_text("M", font).height.max(1);
        self.list_line_cache.set(Some((key, height)));
        height
    }

    /// The column-header row's height: one row in Details mode, zero otherwise (DLG-08).
    fn list_header_height(&self) -> u32 {
        if self.view_mode == ViewMode::Details {
            self.list_row_height()
        } else {
            0
        }
    }

    /// The visible rows and their indices into `entries`, served from a keyed cache (G-P08).
    fn visible_rows(&self) -> Rc<Vec<(usize, Rect)>> {
        let key = VisibleRowsKey {
            list: self.list_rect(),
            row_height: self.list_row_height(),
            header_height: self.list_header_height(),
            scroll: self.scroll,
            entries: self.entries.len(),
        };
        if let Some((cached_key, rows)) = self.visible_cache.borrow().as_ref() {
            if *cached_key == key {
                return Rc::clone(rows);
            }
        }
        let rows = Rc::new(self.compute_visible_rows(&key));
        *self.visible_cache.borrow_mut() = Some((key, Rc::clone(&rows)));
        rows
    }

    /// Rebuilds the visible-row list; [`Self::visible_rows`] is the cached entry point.
    fn compute_visible_rows(&self, key: &VisibleRowsKey) -> Vec<(usize, Rect)> {
        let list = key.list;
        if list.height <= key.header_height || self.entries.is_empty() {
            return Vec::new();
        }
        let row_h = key.row_height.max(1);
        let count = ((list.height - key.header_height) / row_h) as usize;
        let start = self.scroll.min(self.entries.len().saturating_sub(1));
        (0..count)
            .filter_map(|row| {
                let index = start + row;
                self.entries.get(index).map(|_| {
                    (
                        index,
                        Rect::new(
                            list.x,
                            list.y + key.header_height as i32 + row as i32 * row_h as i32,
                            list.width,
                            row_h,
                        ),
                    )
                })
            })
            .collect()
    }

    /// How many rows the list well can show (the header row is excluded).
    fn visible_row_count(&self) -> usize {
        let list = self.list_rect();
        let header = self.list_header_height();
        if list.height <= header {
            return 0;
        }
        ((list.height - header) / self.list_row_height().max(1)) as usize
    }

    /// Selects `target` from the keyboard and keeps it inside the scrolled window (DLG-09).
    fn keyboard_select(&mut self, target: usize) {
        let Some(last) = self.entries.len().checked_sub(1) else {
            return;
        };
        let target = target.min(last);
        self.selected_rows = vec![target];
        self.selected_files = self.row_name(target).into_iter().collect();
        self.selection_changed.emit(self.selected_files.clone());
        let visible = self.visible_row_count().max(1);
        if target < self.scroll {
            self.scroll = target;
        } else if target >= self.scroll + visible {
            self.scroll = target + 1 - visible;
        }
        self.base.request_redraw();
    }

    /// The row index under `pos`, computed arithmetically with the same bands
    /// [`Self::visible_rows`] uses (G-P08).
    fn row_index_at(&self, pos: Point) -> Option<usize> {
        let list = self.list_rect();
        let header = self.list_header_height();
        if list.height <= header || !list.contains_point(pos) {
            return None;
        }
        let row_h = self.list_row_height().max(1);
        let row = ((pos.y - list.y - header as i32) / row_h as i32).max(0) as usize;
        let visible = ((list.height - header) / row_h) as usize;
        let index = self.scroll + row;
        (row < visible && index < self.entries.len()).then_some(index)
    }

    /// The sidebar row that contains `pos`.
    fn sidebar_row_at(&self, pos: Point) -> Option<usize> {
        let width = self.sidebar_width();
        if width == 0 {
            return None;
        }
        let rect = self.frame_rect();
        let list_y = rect.y + dimensions::DIALOG_TITLE_BAR_HEIGHT as i32 + LIST_TOP_GAP;
        let list_h = self.list_rect().height;
        if pos.x < rect.x + 10 || pos.x >= rect.x + 10 + width as i32 {
            return None;
        }
        if pos.y < list_y || pos.y >= list_y + list_h as i32 {
            return None;
        }
        let row_h = self.list_row_height();
        let index = ((pos.y - list_y) / row_h as i32) as usize;
        (index < self.favorites.len()).then_some(index)
    }

    /// Applies a press on the listing: select a row, enter a directory, or report a preview.
    fn press_list_row(&mut self, pos: Point, double_click: bool) {
        let Some(index) = self.row_index_at(pos) else {
            return;
        };
        let Some(entry) = self.entries.get(index).cloned() else { return };
        if double_click {
            self.selected_rows = vec![index];
            self.selected_files = vec![entry.name.clone()];
            self.selection_changed.emit(self.selected_files.clone());
            self.selection_modifier = SelectionModifier::Replace;
            if entry.is_directory {
                let base = self.directory.trim_end_matches(['/', '\\']);
                let separator = if base.is_empty() { "" } else { "/" };
                self.navigate_to(format!("{base}{separator}{}", entry.name));
            } else {
                self.file_selected.emit(entry.name.clone());
                self.quick_look_requested.emit(entry.name);
            }
        } else {
            self.apply_row_selection(index);
        }
        self.base.request_redraw();
    }

    /// Combines a row press with the current selection using the active modifier.
    fn apply_row_selection(&mut self, index: usize) {
        let Some(name) = self.row_name(index) else { return };
        match self.selection_modifier {
            SelectionModifier::Replace => {
                self.selected_rows = vec![index];
                self.selected_files = vec![name];
            }
            SelectionModifier::Ctrl => {
                if let Some(position) = self.selected_rows.iter().position(|row| *row == index) {
                    self.selected_rows.remove(position);
                    self.selected_files.retain(|path| *path != name);
                } else {
                    self.selected_rows.push(index);
                    if !self.selected_files.contains(&name) {
                        self.selected_files.push(name);
                    }
                }
            }
            SelectionModifier::Shift => {
                let anchor = self.selected_rows.last().copied().unwrap_or(index);
                let (low, high) = if anchor <= index { (anchor, index) } else { (index, anchor) };
                self.selected_rows = (low..=high).collect();
                self.selected_files =
                    self.selected_rows.iter().filter_map(|row| self.row_name(*row)).collect();
            }
        }
        self.selection_changed.emit(self.selected_files.clone());
        // The modifier applies to one interaction; the next plain press must replace.
        self.selection_modifier = SelectionModifier::Replace;
    }

    /// The name of the entry at `index`, when the index addresses a row.
    fn row_name(&self, index: usize) -> Option<String> {
        self.entries.get(index).map(|entry| entry.name.clone())
    }

    /// The four column rectangles of a details row, or `None` when the well is too narrow.
    ///
    /// Fixed trailing widths keep the size and date columns aligned across rows; the name
    /// takes whatever is left. Returning `None` rather than squeezing is deliberate: four
    /// two-character columns would be less informative than one readable name.
    fn detail_columns(row: Rect) -> Option<(Rect, Rect, Rect, Rect)> {
        const GAP: u32 = 2;
        const SIZE_W: u32 = 44;
        const MODIFIED_W: u32 = 88;
        const TYPE_W: u32 = 48;
        let x = row.x + 6;
        let width = row.width.saturating_sub(12);
        let trailing = SIZE_W + MODIFIED_W + TYPE_W + GAP * 3;
        let name_width = width.checked_sub(trailing)?;
        if name_width < 48 {
            return None;
        }
        let size_x = x + name_width as i32 + GAP as i32;
        let modified_x = size_x + SIZE_W as i32 + GAP as i32;
        let type_x = modified_x + MODIFIED_W as i32 + GAP as i32;
        Some((
            Rect::new(x, row.y, name_width, row.height),
            Rect::new(size_x, row.y, SIZE_W, row.height),
            Rect::new(modified_x, row.y, MODIFIED_W, row.height),
            Rect::new(type_x, row.y, TYPE_W, row.height),
        ))
    }

    /// Moves the focus ring across the action row and announces it.
    fn move_focus(&mut self) {
        let count = self.action_labels().len();
        let next = super::move_focus(self.focus, count, true);
        if next != self.focus {
            self.focus = next;
            self.focus_changed.emit(self.id());
            self.base.request_redraw();
        }
    }

    /// The action labels this dialog paints, in row order.
    ///
    /// The accept label is mode-dependent, which is why `draw` and the hit test share this
    /// function rather than each spelling the conditional out.
    fn action_labels(&self) -> Vec<String> {
        let ok_label = if self.mode == FileDialogMode::SaveFile {
            super::localized_label("common.button.save", "Save")
        } else {
            super::localized_label("common.button.open", "Open")
        };
        vec![ok_label, super::localized_label("common.button.cancel", "Cancel")]
    }

    /// The action button rects, rebuilt from the same frame and bottom band `draw` uses.
    ///
    /// The press path has no render context of its own, so it builds a measuring one — the
    /// pattern `MessageBox::action_button_at` established. Rebuilding rather than caching is
    /// deliberate: a cached rect is a second copy of the geometry that a resize can leave stale.
    fn action_buttons(&self) -> Vec<Rect> {
        let size = Size::new(self.base.geometry().width.max(1), self.base.geometry().height.max(1));
        let mut backend = crate::render::SoftwarePaintBackend::new(size, 1.0);
        let context = RenderContext::new(&mut backend);
        let rect = self.frame_rect();
        let button_band = super::message_box::inset_action_band(
            ControlMetrics::bottom_band(rect, dimensions::DIALOG_BUTTON_HEIGHT),
            rect.y,
        );
        let labels = self.action_labels();
        super::message_box::action_row_geometry(
            &context,
            effective_font(self.base.style()),
            &labels,
            button_band,
            true,
        )
        .buttons
    }

    /// The index of the action button under `pos`, or `None`.
    fn button_at(&self, pos: Point) -> Option<usize> {
        self.action_buttons().iter().position(|button| button.contains_point(pos))
    }
}

impl Draw for FileDialog {
    fn draw(&mut self, context: &mut RenderContext) {
        // A modal dialog dims the page behind it: that dimming is the whole visual difference
        // between "this dialog is up and the page is inert" and "a panel was drawn on the page".
        // The `modal` flag was recorded and enforced by the modal stack, and nothing painted it.
        if self.modal {
            super::draw_modal_scrim(context, self.geometry());
        }
        // The **frame**, not the control's rectangle: see `frame_rect`.
        let rect = self.frame_rect();
        let style = self.style();
        // The **effective font** — the resolved theme/caller font — for the title, placeholder,
        // labels, file name and buttons, so the dialog honours the theme body font and the user's
        // text scale (D09-STYLE-01).
        let font = effective_font(style);

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
        let (window_fill, accent, theme_ink, theme_border) = {
            let manager = crate::style::theme_manager();
            match manager.current_theme() {
                Some(active) => (
                    active.colors.background,
                    active.colors.primary,
                    Some(active.colors.foreground),
                    Some(active.colors.outline),
                ),
                None => (Color::WHITE, Color::rgb(0, 120, 215), None, None),
            }
        };
        let accent_ink = accent.contrast_color();

        // A dialog is a `Surface`-role control and `Surface` resolves to the window's own
        // fill, which would leave the frame invisible against the window. A resolved
        // surface equal to the window fill is therefore re-derived one step toward the
        // ink, the same distinction `Colors::input_background` draws for a field.
        let ink = style.text_color.or(theme_ink).unwrap_or(Color::BLACK);
        let surface = match style.background_color {
            Some(resolved) if resolved != window_fill => resolved,
            _ => window_fill.blend(&ink, 0.06),
        };
        let border = style.border_color.or(theme_border).unwrap_or(Color::rgb(160, 160, 160));
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
            context.face_with_gradient(
                rect,
                surface,
                self.style().background_gradient.as_ref(),
                self.style().surface.unwrap_or_default(),
                self.style().border_radius.unwrap_or(0),
                Color::BLACK,
            );
            context.draw_rect(rect, border);
        }
        // Title bar: a separate region from the dialog surface, derived one visible step from
        // it rather than filled with the accent. Every other dialog in the crate (message box,
        // wizard, progress, colour) draws a neutral title strip; the accent-filled bar made this
        // one read as a different class of window — a blue header is not macOS chrome. The accent
        // is still the accept button's fill below, which is where a call to action belongs.
        let title_bar = surface.blend(&ink, 0.08);
        let title_bar_band = ControlMetrics::top_band(rect, dimensions::DIALOG_TITLE_BAR_HEIGHT);
        context.fill_rect(title_bar_band, title_bar);
        if !self.title.is_empty() {
            let title_font = font;
            let title_line = context.text_line(title_bar_band, title_font);
            context.draw_text_fitted_cow(
                Rect::new(
                    rect.x + 8,
                    title_line.y,
                    rect.width.saturating_sub(16),
                    title_line.height.max(1),
                ),
                &self.title,
                title_font,
                ink,
                HorizontalAlignment::Left,
            );
        }
        // The three stacked rows are the frame's own bands: the button row at the bottom,
        // the selected-file strip above it, and the list taking the remainder. Deriving all
        // three from the frame is what keeps them from overlapping on a short dialog — each
        // was previously placed from an absolute offset written for a taller one.
        let button_band = super::message_box::inset_action_band(
            ControlMetrics::bottom_band(rect, dimensions::DIALOG_BUTTON_HEIGHT),
            rect.y,
        );
        let button_top = button_band.y;
        let list_rect = self.list_rect();
        let list_y = list_rect.y;
        let list_h = list_rect.height;
        // The sidebar is a host-supplied favorite list; with none, its width is zero and the
        // list keeps the full frame, so the default appearance is unchanged.
        let sidebar_width = self.sidebar_width();
        if sidebar_width > 0 && list_h > 0 {
            let sidebar = Rect::new(rect.x + 10, list_y, sidebar_width, list_h);
            context.fill_rect(sidebar, field);
            context.draw_rect(sidebar, border);
            context.push_clip(sidebar.x, sidebar.y, sidebar.width, sidebar.height);
            let row_h = self.list_row_height();
            for (index, favorite) in self.favorites.iter().enumerate() {
                let row = Rect::new(
                    sidebar.x,
                    sidebar.y + index as i32 * row_h as i32,
                    sidebar.width,
                    row_h,
                );
                if row.y >= sidebar.y + sidebar.height as i32 {
                    break;
                }
                let band = Rect::new(row.x + 6, row.y, row.width.saturating_sub(12), row.height);
                context.draw_text_fitted_cow(band, favorite, font, ink, HorizontalAlignment::Left);
            }
            context.pop_clip();
        }
        if list_rect.height > 0 {
            context.fill_rect(list_rect, field);
            context.draw_rect(list_rect, border);
        }
        if !self.entries.is_empty() && list_rect.height > 0 {
            // Only the visible rows are painted: a directory with thousands of entries costs
            // the rows on screen, not the rows in the model.
            context.push_clip(list_rect.x, list_rect.y, list_rect.width, list_rect.height);
            // The Details header is the first row of the list well, and the data rows begin
            // below it (DLG-08). It is clickable so the header drives the sort.
            let header_h = self.list_header_height();
            if header_h > 0 {
                let header = Rect::new(list_rect.x, list_rect.y, list_rect.width, header_h);
                context.fill_rect(header, surface.blend(&ink, 0.10));
                if let Some((name, size, modified, kind)) = Self::detail_columns(header) {
                    let header_line = context.text_line(header, font);
                    for (column, band) in [
                        (Some(SortColumn::Name), name),
                        (Some(SortColumn::Size), size),
                        (Some(SortColumn::Modified), modified),
                        (Some(SortColumn::Type), kind),
                    ] {
                        let Some(column) = column else { continue };
                        let label = match column {
                            SortColumn::Name => {
                                super::localized_label("dialog.file_dialog.column_name", "Name")
                            }
                            SortColumn::Size => {
                                super::localized_label("dialog.file_dialog.column_size", "Size")
                            }
                            SortColumn::Modified => super::localized_label(
                                "dialog.file_dialog.column_modified",
                                "Modified",
                            ),
                            SortColumn::Type => {
                                super::localized_label("dialog.file_dialog.column_type", "Type")
                            }
                        };
                        let active = self.sort_column == Some(column);
                        let label_ink = if active { accent } else { ink };
                        let text_band = Rect::new(
                            band.x,
                            header_line.y,
                            band.width.saturating_sub(if active { 12 } else { 0 }).max(1),
                            header_line.height.max(1),
                        );
                        context.draw_text_fitted_cow(
                            text_band,
                            &label,
                            font,
                            label_ink,
                            HorizontalAlignment::Left,
                        );
                        if active {
                            // The direction mark is an icon outline rather than a text glyph,
                            // so both backends draw the same shape.
                            let mark =
                                Rect::new(band.x + band.width as i32 - 10, band.y, 10, header_h);
                            let icon = if self.sort_descending {
                                IconName::ChevronDown
                            } else {
                                IconName::ChevronUp
                            };
                            crate::widget::draw_icon_centered(context, mark, 8, accent, icon);
                        }
                    }
                }
            }
            for (index, row) in self.visible_rows().iter().copied() {
                if self.selected_rows.contains(&index) {
                    context.fill_rect(row, accent.with_alpha_f32(0.18));
                }
                let Some(entry) = self.entries.get(index) else { continue };
                // The host's icon occupies the row's leading gutter; the name starts after it, so
                // an icon and a name cannot overlap. A row without an icon keeps the historical
                // text position exactly.
                let icon_pad = match entry.icon {
                    Some(source) => {
                        const ICON: u32 = 16;
                        let slot = Rect::new(
                            row.x + 4,
                            row.y + row.height.saturating_sub(ICON) as i32 / 2,
                            ICON,
                            ICON,
                        );
                        let _ =
                            crate::widget::paint_icon(context, IconRef::from(source), slot, ink);
                        ICON
                    }
                    None => 0,
                };
                let band = Rect::new(
                    row.x + 6 + icon_pad as i32,
                    row.y,
                    row.width.saturating_sub(12 + icon_pad),
                    row.height,
                );
                let line = context.text_line(band, font);
                let text_band = Rect::new(band.x, line.y, band.width.max(1), line.height.max(1));
                match self.view_mode {
                    ViewMode::List => {
                        context.draw_text_fitted_cow(
                            text_band,
                            &entry.name,
                            font,
                            if entry.is_directory { accent } else { ink },
                            HorizontalAlignment::Left,
                        );
                    }
                    ViewMode::Details => {
                        // Three columns after the name. They are omitted (and the name keeps
                        // the whole row) when the well is too narrow to hold them without
                        // eliding every one to nothing.
                        let columns = Self::detail_columns(row);
                        match columns {
                            Some((name_col, size_col, modified_col, type_col)) => {
                                // The name column loses the icon gutter, so the icon never overlaps
                                // the name it belongs to.
                                let name_col = Rect::new(
                                    name_col.x,
                                    name_col.y,
                                    name_col.width.saturating_sub(icon_pad),
                                    name_col.height,
                                );
                                context.draw_text_fitted_cow(
                                    Rect::new(
                                        name_col.x,
                                        line.y,
                                        name_col.width.max(1),
                                        line.height.max(1),
                                    ),
                                    &entry.name,
                                    font,
                                    if entry.is_directory { accent } else { ink },
                                    HorizontalAlignment::Left,
                                );
                                let size = entry.size.map(format_size).unwrap_or_default();
                                let modified =
                                    entry.modified.map(format_modified).unwrap_or_default();
                                let kind = entry.kind.label();
                                context.draw_text_fitted_cow(
                                    Rect::new(
                                        size_col.x,
                                        line.y,
                                        size_col.width.max(1),
                                        line.height.max(1),
                                    ),
                                    &size,
                                    font,
                                    ink.with_alpha_f32(0.75),
                                    HorizontalAlignment::Right,
                                );
                                context.draw_text_fitted_cow(
                                    Rect::new(
                                        modified_col.x,
                                        line.y,
                                        modified_col.width.max(1),
                                        line.height.max(1),
                                    ),
                                    &modified,
                                    font,
                                    ink.with_alpha_f32(0.75),
                                    HorizontalAlignment::Left,
                                );
                                context.draw_text_fitted_cow(
                                    Rect::new(
                                        type_col.x,
                                        line.y,
                                        type_col.width.max(1),
                                        line.height.max(1),
                                    ),
                                    kind,
                                    font,
                                    ink.with_alpha_f32(0.75),
                                    HorizontalAlignment::Left,
                                );
                            }
                            None => {
                                context.draw_text_fitted_cow(
                                    text_band,
                                    &entry.name,
                                    font,
                                    if entry.is_directory { accent } else { ink },
                                    HorizontalAlignment::Left,
                                );
                            }
                        }
                    }
                }
            }
            // A scroll indicator on the list's trailing edge: the thumb is sized by the visible /
            // total ratio and positioned by the scroll offset (DLG-19).
            let visible_count = self.visible_rows().len();
            if self.entries.len() > visible_count && visible_count > 0 && list_rect.height > 0 {
                let track = Rect::new(
                    list_rect.x + list_rect.width as i32 - 3,
                    list_rect.y + header_h as i32,
                    3,
                    list_rect.height - header_h,
                );
                context.fill_rect(track, border.with_alpha_f32(0.35));
                let thumb_h =
                    ((track.height as usize * visible_count) / self.entries.len()).max(8) as u32;
                let max_scroll = self.entries.len().saturating_sub(visible_count);
                let travel = track.height.saturating_sub(thumb_h) as usize;
                let thumb_y =
                    track.y + (travel * self.scroll).checked_div(max_scroll).unwrap_or(0) as i32;
                context.fill_rect(Rect::new(track.x, thumb_y, 3, thumb_h), accent);
            }
            context.pop_clip();
        } else if self.entries.is_empty() {
            // The placeholder lives inside the list it describes, so it truncates at the
            // list's edge rather than at a coordinate chosen for a wider default size. Its
            // line box is centred in the list via the shared primitive: the glyph origin is
            // the box's top-left, so the old fixed `list_rect.y + 6` placed a 14 px line
            // from y=44 to 58 inside a list ending at 46 — twelve pixels of the text that
            // explains the empty list were outside the well it belongs to. Drawn only when the
            // list is non-empty, so a squeezed dialog emits no `<text …></text>`.
            let placeholder = tr!("dialog.file_dialog.file_list_placeholder");
            let placeholder_font = font;
            if list_rect.height > 0 && !placeholder.is_empty() {
                let placeholder_band = Rect::new(
                    list_rect.x + 6,
                    list_rect.y,
                    list_rect.width.saturating_sub(12),
                    list_rect.height,
                );
                let placeholder_line = context.text_line(placeholder_band, placeholder_font);
                context.draw_text_fitted_cow(
                    Rect::new(
                        placeholder_band.x,
                        placeholder_line.y,
                        placeholder_band.width.max(1),
                        placeholder_line.height.max(1),
                    ),
                    &placeholder,
                    placeholder_font,
                    // Dimmed toward the field, then held to the text floor: a fixed 50% blend
                    // measured 3.65:1 on the light field, so the line explaining what the empty
                    // list is for was itself hard to read.
                    ink.blend(&field, 0.5).legible_on(field, 4.5),
                    HorizontalAlignment::Left,
                );
            }
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
            let sel_label = if self.label_text.is_empty() {
                tr!("dialog.file_dialog.file_name")
            } else {
                self.label_text.clone()
            };
            let sel_label_font = font;
            let sel_band = Rect::new(rect.x + 10, sel_y, 66, sel_h as u32);
            let sel_line = context.text_line(sel_band, sel_label_font);
            context.draw_text_fitted_cow(
                Rect::new(sel_band.x, sel_line.y, sel_band.width, sel_line.height.max(1)),
                &sel_label,
                sel_label_font,
                ink,
                HorizontalAlignment::Left,
            );
            let fname = self.display_name();
            let fname_rect =
                Rect::new(rect.x + 80, sel_y, rect.width.saturating_sub(90), sel_h as u32);
            context.fill_rect(fname_rect, field);
            context.draw_rect(fname_rect, border);
            // Guarded: an unguarded draw of an empty file name emits `<text …></text>`.
            if !fname.is_empty() {
                let fname_font = font;
                let fname_band = Rect::new(
                    fname_rect.x + 4,
                    fname_rect.y,
                    fname_rect.width.saturating_sub(8),
                    fname_rect.height,
                );
                let fname_line = context.text_line(fname_band, fname_font);
                context.draw_text_fitted_cow(
                    fname_line,
                    fname,
                    fname_font,
                    ink,
                    HorizontalAlignment::Left,
                );
                if self.name_editing {
                    // A typing caret after the committed text, so the field reads as editable.
                    let caret_x = fname_line.x
                        + crate::widget::metrics::estimate_text_width(fname, fname_font, 1.0)
                            as i32;
                    context.fill_rect(
                        Rect::new(
                            caret_x.min(fname_band.x + fname_band.width as i32 - 1),
                            fname_line.y,
                            1,
                            fname_line.height.max(1),
                        ),
                        ink,
                    );
                }
            }
        } else {
            // The file name still needs a home; it moves into the list's own band, which
            // is what the list is for. This keeps the control's content reachable at a
            // size where the strip has no room, rather than silently dropping it.
            let fname = self.display_name();
            let fname_font = font;
            let band = Rect::new(
                list_rect.x + 6,
                list_rect.y,
                list_rect.width.saturating_sub(12),
                list_rect.height,
            );
            if !fname.is_empty() && band.height > 0 {
                let line = context.text_line(band, fname_font);
                context.draw_text_fitted_cow(
                    line,
                    fname,
                    fname_font,
                    ink,
                    HorizontalAlignment::Left,
                );
            }
        }
        // OK/Cancel buttons. The pair is right-aligned inside the frame through the shared
        // [`action_row_geometry`] derivation, so this dialog and the seven others that draw the
        // same pair cannot disagree about the button width, the gap between them or where the
        // row's left edge falls. The row is the frame's bottom band, and the labels are centred
        // in their buttons and fitted to them, so a truncating locale cannot spill out of the
        // button it belongs to.
        let labels = self.action_labels();
        let row =
            super::message_box::action_row_geometry(context, font, &labels, button_band, true);
        // The accept button is the dialog's call to action: the theme's accent, with its
        // contrast colour as the label — the same pairing `WidgetRole::Primary` uses.
        let ok_rect = row.buttons[0];
        context.fill_rect(ok_rect, accent);
        context.draw_text_line_cow(
            ok_rect,
            &labels[0],
            font,
            accent_ink,
            HorizontalAlignment::Center,
        );
        let cancel_rect = row.buttons[1];
        context.fill_rect(cancel_rect, surface.blend(&ink, 0.1));
        context.draw_rect(cancel_rect, border);
        context.draw_text_line_cow(cancel_rect, &labels[1], font, ink, HorizontalAlignment::Center);
        if let Some(focused) = self.focus.and_then(|index| row.buttons.get(index)) {
            context.draw_rounded_rect_stroke(*focused, 2, accent, 2);
        }
    }
}

/// A byte count in the shortest unit that keeps it readable.
fn format_size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit = 0usize;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// A Unix timestamp as `YYYY-MM-DD` in UTC.
///
/// The details view only needs a stable, legible rendering of the host's number. A local-time
/// conversion would need the platform's zone database, which this library deliberately does not
/// read from a widget; UTC is the honest spelling of the value the host supplied. The date alone
/// is what fits the dialog's 280 px frame — a full time of day would be elided mid-string and
/// therefore less informative than the day it belongs to.
fn format_modified(seconds: u64) -> String {
    let days = (seconds / 86_400) as i64;
    let (year, month, day) = civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02}")
}

/// Converts days since 1970-01-01 to a Gregorian `(year, month, day)`.
///
/// Howard Hinnant's `civil_from_days`, the standard branch-free inverse of the day-count
/// algorithm. It is here rather than in a date module because the dialog is the only caller
/// and pulling in a calendar dependency for one cell would be disproportionate.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = z - era * 146_097; // [0, 146096]
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * month_prime + 2) / 5 + 1) as u32;
    let month = (if month_prime < 10 { month_prime + 3 } else { month_prime - 9 }) as u32;
    if month <= 2 {
        year += 1;
    }
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Point;
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

    /// A **mounted** dialog must accept the selection the host's own list produced.
    ///
    /// Once registered, a host holds an `ObjectId`, so the `&mut FileDialog` setter is out of
    /// reach; the property route is the only way back in, and it used to refuse with
    /// `ReadOnlyProperty`. This pins the write, the clear (`Null`), and the signal that tells the
    /// host the write landed.
    #[test]
    fn a_mounted_dialog_accepts_a_selection_written_through_the_property_route() {
        use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
        use crate::widget::capability::WidgetProperties;

        let mut dialog = FileDialog::new(Rect::new(0, 0, 420, 280));
        assert_eq!(dialog.selected_file(), None);

        let seen = Arc::new(Mutex::new(Vec::<String>::new()));
        let seen_clone = Arc::clone(&seen);
        dialog.file_selected.connect(move |path| {
            if let Ok(mut v) = seen_clone.lock() {
                v.push((*path).clone());
            }
        });

        dialog
            .set("selected_file", CapabilityValue::String("/tmp/picked.txt".to_string()))
            .expect("a host-owned list must be able to write the chosen path");
        assert_eq!(dialog.selected_file(), Some("/tmp/picked.txt"));
        assert_eq!(
            *seen.lock().expect("selected lock"),
            vec!["/tmp/picked.txt".to_string()],
            "the write must announce itself the same way the setter does"
        );

        // `Null` clears without closing the dialog.
        dialog.set("selected_file", CapabilityValue::Null).expect("clearing is a valid write");
        assert_eq!(dialog.selected_file(), None);
        assert!(dialog.is_visible(), "clearing the selection must not dismiss the dialog");

        // A wrong carrier type is still the caller's error, not a silent drop.
        assert_eq!(
            dialog.set("selected_file", CapabilityValue::UInt(3)),
            Err(CapabilityAccessError::TypeMismatch)
        );
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
    fn clicking_ok_accepts_and_clicking_cancel_rejects() {
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

        dialog.select_file("/tmp/clicked.txt");
        dialog.show();
        let ok = dialog.action_buttons()[0].center();
        dialog.handle_event(&Event::mouse_press(ok.x, ok.y, 1));
        assert_eq!(*accepted.lock().expect("accepted lock"), 1);
        assert!(!dialog.is_visible());

        dialog.show();
        let cancel = dialog.action_buttons()[1].center();
        dialog.handle_event(&Event::mouse_press(cancel.x, cancel.y, 1));
        assert_eq!(*rejected.lock().expect("rejected lock"), 1);
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

    /// A host-owned listing: the dialog never touches the filesystem itself.
    struct FakeSource {
        path_exists: bool,
    }

    impl FileEntrySource for FakeSource {
        fn entries(&self, directory: &str) -> Vec<FileEntry> {
            if directory == "/docs" {
                vec![FileEntry::new("another.txt", false)]
            } else {
                vec![
                    FileEntry::new("src", true),
                    FileEntry::new("a.txt", false),
                    FileEntry::new("b.txt", false),
                ]
            }
        }

        fn exists(&self, _path: &str) -> bool {
            self.path_exists
        }
    }

    fn dialog_with_source() -> FileDialog {
        let mut dialog = FileDialog::new(Rect::new(0, 0, 420, 320));
        dialog.set_source(Box::new(FakeSource { path_exists: false }));
        dialog
    }

    #[test]
    fn the_host_listing_is_drawn_and_selectable() {
        let mut dialog = dialog_with_source();
        assert_eq!(dialog.entries().len(), 3);
        let list = dialog.list_rect();
        dialog.handle_event(&Event::mouse_press(list.x + 4, list.y + 2, 1));
        assert_eq!(dialog.selected_row(), Some(0));
        assert_eq!(dialog.selected_file(), Some("src"));
        let svg = crate::widget::svg::render_to_svg(&mut dialog);
        assert!(svg.contains("data-text"), "the listing must paint its rows: {svg}");
    }

    #[test]
    fn the_wheel_scrolls_the_listing() {
        let mut dialog = FileDialog::new(Rect::new(0, 0, 420, 200));
        dialog.set_source(Box::new(FakeSource { path_exists: false }));
        let list = dialog.list_rect();
        let visible = (list.height / dialog.list_row_height()) as usize;
        if visible < dialog.entries().len() {
            dialog.handle_event(&Event::Wheel { delta: Point::new(0, 1), modifiers: 0 });
            assert!(dialog.scroll_offset() > 0);
        }
    }

    #[test]
    fn a_save_appends_the_default_suffix() {
        let mut dialog = FileDialog::save_file(Rect::new(0, 0, 420, 320));
        dialog.set_default_suffix(".txt");
        dialog.select_file("/tmp/report");
        dialog.accept();
        assert_eq!(dialog.selected_file(), Some("/tmp/report.txt"));
    }

    #[test]
    fn a_save_asks_before_overwriting_an_existing_path() {
        let mut dialog = FileDialog::save_file(Rect::new(0, 0, 420, 320));
        dialog.set_source(Box::new(FakeSource { path_exists: true }));
        dialog.set_overwrite_confirmation(true);
        dialog.select_file("/tmp/existing.txt");
        let asked = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = Arc::clone(&asked);
        dialog.overwrite_requested.connect(move |path| {
            sink.lock().expect("lock").push((*path).clone());
        });
        dialog.show();
        dialog.accept();
        assert_eq!(*asked.lock().expect("lock"), vec!["/tmp/existing.txt".to_string()]);
        assert!(dialog.is_visible(), "the dialog waits for the answer");
        assert_eq!(dialog.pending_overwrite(), Some("/tmp/existing.txt"));
        dialog.confirm_overwrite();
        assert!(!dialog.is_visible());
        assert_eq!(dialog.pending_overwrite(), None);
    }

    #[test]
    fn history_moves_back_and_forward() {
        let mut dialog = dialog_with_source();
        dialog.navigate_to("/docs");
        assert_eq!(dialog.directory(), "/docs");
        assert!(dialog.can_go_back());
        dialog.go_back();
        assert_eq!(dialog.directory(), "");
        assert!(dialog.can_go_forward());
        dialog.go_forward();
        assert_eq!(dialog.directory(), "/docs");
    }

    #[test]
    fn a_sidebar_favorite_navigates() {
        let mut dialog = dialog_with_source();
        dialog.set_sidebar(vec!["/docs".to_string()]);
        assert_eq!(dialog.sidebar_width(), 96);
        let rect = dialog.frame_rect();
        let list_y = rect.y + dimensions::DIALOG_TITLE_BAR_HEIGHT as i32 + LIST_TOP_GAP;
        dialog.handle_event(&Event::mouse_press(rect.x + 14, list_y + 2, 1));
        assert_eq!(dialog.directory(), "/docs");
    }

    #[test]
    fn a_control_click_adds_to_the_selection_and_announces_it() {
        let mut dialog = dialog_with_source();
        let seen = Arc::new(Mutex::new(Vec::<Vec<String>>::new()));
        let sink = Arc::clone(&seen);
        dialog.selection_changed.connect(move |paths| {
            sink.lock().expect("lock").push(paths.as_ref().clone());
        });
        let list = dialog.list_rect();
        // Row 1 is `a.txt`, row 2 is `b.txt`.
        let row_y = list.y + dialog.list_row_height() as i32;
        dialog.handle_event(&Event::mouse_press_with(
            list.x + 4,
            row_y,
            1,
            crate::shortcut::Modifiers::CTRL.bits() as u32,
        ));
        assert_eq!(dialog.selected_rows(), &[1]);
        dialog.handle_event(&Event::mouse_press_with(
            list.x + 4,
            list.y + 2 * dialog.list_row_height() as i32,
            1,
            crate::shortcut::Modifiers::CTRL.bits() as u32,
        ));
        assert_eq!(dialog.selected_files(), ["a.txt".to_string(), "b.txt".to_string()]);
        assert_eq!(
            *seen.lock().expect("lock"),
            vec![vec!["a.txt".to_string()], vec!["a.txt".to_string(), "b.txt".to_string()]]
        );

        dialog.select_files(vec!["b.txt".to_string()]);
        assert!(dialog.add_selected_file("src"), "the append path accepts a second row");
        assert_eq!(
            dialog.selected_files(),
            ["b.txt".to_string(), "src".to_string()],
            "the append path builds a multi-row selection"
        );
    }

    #[test]
    fn the_details_view_and_sorting_are_reachable_through_the_property_route() {
        use crate::widget::capability::types::CapabilityValue;
        use crate::widget::capability::WidgetProperties;

        struct SizedSource;
        impl FileEntrySource for SizedSource {
            fn entries(&self, _directory: &str) -> Vec<FileEntry> {
                vec![
                    FileEntry::new("big.bin", false).with_metadata(Some(2048), Some(0)),
                    FileEntry::new("small.txt", false).with_metadata(Some(512), Some(0)),
                    FileEntry::new("dir", true),
                ]
            }
        }

        let mut dialog = FileDialog::new(Rect::new(0, 0, 420, 320));
        dialog.set_source(Box::new(SizedSource));
        assert_eq!(dialog.view_mode(), ViewMode::List);
        dialog.set("view_mode", CapabilityValue::String("details".to_string())).expect("writable");
        assert_eq!(dialog.view_mode(), ViewMode::Details);
        assert_eq!(
            dialog.get("view_mode").expect("readable"),
            CapabilityValue::String("details".to_string())
        );
        assert_eq!(
            dialog.set("view_mode", CapabilityValue::String("nope".to_string())),
            Err(crate::widget::capability::CapabilityAccessError::OutOfRange)
        );

        dialog.sort_by(SortColumn::Size);
        let names: Vec<&str> = dialog.entries().iter().map(|entry| entry.name.as_str()).collect();
        assert_eq!(names, ["dir", "small.txt", "big.bin"]);
        assert_eq!(format_size(512), "512 B");
        assert_eq!(format_modified(0), "1970-01-01");
        let columns = FileDialog::detail_columns(Rect::new(0, 0, 260, 18))
            .expect("the census-width well holds the four columns");
        assert!(columns.3.width > 0, "the type column keeps a visible width");

        let list_svg = {
            dialog.set_view_mode(ViewMode::List);
            crate::widget::svg::render_to_svg(&mut dialog)
        };
        dialog.set_view_mode(ViewMode::Details);
        let details_svg = crate::widget::svg::render_to_svg(&mut dialog);
        assert!(
            details_svg.matches("data-text").count() > list_svg.matches("data-text").count(),
            "the details view paints its metadata columns as extra text runs"
        );
    }

    #[test]
    fn navigating_reports_the_directory_and_history_can_be_restored() {
        let mut dialog = dialog_with_source();
        let entered = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = Arc::clone(&entered);
        dialog.directory_entered.connect(move |directory| {
            sink.lock().expect("lock").push(directory.as_ref().clone());
        });
        dialog.navigate_to("/docs");
        assert_eq!(*entered.lock().expect("lock"), vec!["/docs".to_string()]);

        dialog.set_history(vec!["/a".to_string(), "/b".to_string()]);
        assert_eq!(dialog.history(), ["/a".to_string(), "/b".to_string()]);
        assert!(dialog.can_go_back(), "the restored cursor sits at the most recent entry");
        dialog.go_back();
        assert_eq!(dialog.directory(), "/a");

        dialog.set_sidebar(vec!["/docs".to_string()]);
        let urls = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = Arc::clone(&urls);
        dialog.url_selected.connect(move |url| {
            sink.lock().expect("lock").push(url.as_ref().clone());
        });
        let rect = dialog.frame_rect();
        let list_y = rect.y + dimensions::DIALOG_TITLE_BAR_HEIGHT as i32 + LIST_TOP_GAP;
        dialog.handle_event(&Event::mouse_press(rect.x + 14, list_y + 2, 1));
        assert_eq!(*urls.lock().expect("lock"), vec!["/docs".to_string()]);
    }

    #[test]
    fn sidebar_urls_round_trip_through_the_property_route() {
        use crate::widget::capability::types::CapabilityValue;
        use crate::widget::capability::WidgetProperties;

        let mut dialog = FileDialog::new(Rect::new(0, 0, 420, 280));
        let urls = vec!["/one".to_string(), "/two".to_string()];
        let list =
            CapabilityValue::Tuple(urls.iter().cloned().map(CapabilityValue::String).collect());
        dialog.set("sidebar_urls", list).expect("the sidebar is writable");
        assert_eq!(dialog.sidebar(), urls.as_slice());
        assert_eq!(
            dialog.get("sidebar_urls").expect("readable"),
            CapabilityValue::Tuple(urls.into_iter().map(CapabilityValue::String).collect())
        );
    }

    #[test]
    fn tab_moves_the_focus_ring_and_announces_it() {
        let mut dialog = FileDialog::new(Rect::new(0, 0, 420, 280));
        let seen = Arc::new(Mutex::new(Vec::<u64>::new()));
        let sink = Arc::clone(&seen);
        dialog.focus_changed.connect(move |id| {
            sink.lock().expect("lock").push(*id);
        });
        dialog.handle_event(&Event::key_press(key_codes::TAB, 0));
        assert_eq!(seen.lock().expect("lock").len(), 1, "Tab must move the ring");
    }

    /// The details model carries a kind and an icon, the type column sorts by kind, and the sort
    /// announces the column it applied.
    #[test]
    fn file_entries_carry_kind_and_icon_and_sort_type_is_announced() {
        struct KindedSource;
        impl FileEntrySource for KindedSource {
            fn entries(&self, _directory: &str) -> Vec<FileEntry> {
                vec![
                    FileEntry::new("z.txt", false)
                        .with_kind(FileKind::Symlink)
                        .with_icon(IconSource::Builtin(crate::widget::IconName::File)),
                    FileEntry::new("dir", true),
                    FileEntry::new("a.txt", false).with_kind(FileKind::File),
                ]
            }
        }

        let mut dialog = FileDialog::new(Rect::new(0, 0, 420, 320));
        dialog.set_source(Box::new(KindedSource));
        assert_eq!(dialog.entries()[0].kind, FileKind::Symlink);
        assert!(dialog.entries()[0].icon.is_some(), "the host's icon is retained");
        assert_eq!(dialog.entries()[1].kind, FileKind::Directory);

        let seen = Arc::new(Mutex::new(Vec::<(SortColumn, bool)>::new()));
        let sink = Arc::clone(&seen);
        dialog.sort_changed.connect(move |payload| {
            sink.lock().expect("sort lock").push(*payload);
        });
        dialog.sort_by(SortColumn::Type);

        let names: Vec<&str> = dialog.entries().iter().map(|entry| entry.name.as_str()).collect();
        assert_eq!(names, ["dir", "a.txt", "z.txt"], "directories, files, then links");
        assert_eq!(*seen.lock().expect("sort lock"), vec![(SortColumn::Type, false)]);

        // The details view paints the kind column and the icon; rendering it must not panic, and
        // it adds the metadata text runs the list view does not have. Text is rendered as path
        // geometry, so the labels themselves are asserted on the enum rather than in the SVG.
        assert_eq!(FileKind::Directory.label(), "Folder");
        assert_eq!(FileKind::Symlink.label(), "Link");
        let list_svg = {
            dialog.set_view_mode(ViewMode::List);
            crate::widget::svg::render_to_svg(&mut dialog)
        };
        dialog.set_view_mode(ViewMode::Details);
        let details_svg = crate::widget::svg::render_to_svg(&mut dialog);
        assert!(
            crate::widget::svg::text_subpath_count(&details_svg)
                > crate::widget::svg::text_subpath_count(&list_svg),
            "the details view paints the type column as extra text runs"
        );
    }
}
