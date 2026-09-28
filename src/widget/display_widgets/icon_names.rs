// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//
// GENERATED FILE — DO NOT EDIT BY HAND.
// Produced by `tools/gen_icon_names.py` from `tools/icon_tokens.txt`. Regenerate with:
//     python3 tools/gen_icon_names.py
//
// The `IconName` type: the enum, its token spellings, and the accessors the rest of the
// crate uses. The token list is the single source of truth; the vendor map
// (`tools/vendor_material_symbols.py`) is cross-checked against it, so a token that
// cannot be drawn or a vendored icon nothing can name is a generation failure.
//
// The geometry itself lives in `icon_data.rs` (opt-in `icons` feature) and
// `icon_fallback_data.rs` (always), both indexed by this enum's declaration order.

// `IconData` is defined once, in `icon.rs`, so a build without the `icons` feature can still
// name the type; this module only borrows it for the two accessors that return it.
use super::IconData;

/// Common icon names for use with the Icon widget.
///
/// Every variant names an icon this crate ships. With the opt-in `icons` feature on it
/// resolves to a real Material Symbols outline; with the feature off it resolves to
/// generated fallback geometry derived from the same outline. Either way the picture is a
/// faithful shape rather than an alias of another icon — every variant round-trips through
/// [`IconName::as_str`] and [`IconName::from_name`], and the integrity tests assert no two
/// variants draw the same picture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconName {
    /// A tick, for confirmation or success.
    Check,
    /// Two crossing strokes inside a circle, for cancel or "no".
    Cross,
    /// A left-pointing arrow, for "back".
    ArrowLeft,
    /// A right-pointing arrow, for "next".
    ArrowRight,
    /// An upward arrow.
    ArrowUp,
    /// A downward arrow.
    ArrowDown,
    /// A five-pointed star, for favourites or ratings.
    Star,
    /// A heart, for likes or favourites.
    Heart,
    /// A gear, for configuration.
    Settings,
    /// A house, for the home view.
    Home,
    /// A magnifying glass, for search.
    Search,
    /// Three stacked bars, for a navigation menu.
    Menu,
    /// The X-shaped dismiss mark.
    Close,
    /// A plus sign, for adding.
    Plus,
    /// A minus sign, for removing.
    Minus,
    /// The letter "i" in a circle, for informational messages.
    Info,
    /// A triangle with an exclamation mark, for warnings.
    Warning,
    /// A circle with an exclamation mark, for errors.
    Error,
    /// A head-and-shoulders silhouette, for an account.
    User,
    /// An envelope, for messages.
    Mail,
    /// A bell, for notifications.
    Bell,
    /// A pencil, for editing.
    Edit,
    /// A waste bin, for deletion.
    Trash,
    /// A node-and-branches glyph, for sharing.
    Share,
    /// A circular arrow, for reloading.
    Refresh,
    /// Three horizontal dots, for an overflow menu.
    More,
    /// A funnel, for filtering.
    Filter,
    /// A closed padlock.
    Lock,
    /// An open padlock.
    Unlock,
    /// A downward arrow into a tray, for downloading.
    Download,
    /// An upward arrow out of a tray, for uploading.
    Upload,
    /// A left chevron, for collapsing or stepping back.
    ChevronLeft,
    /// A right chevron, for expanding or stepping forward.
    ChevronRight,
    /// An up chevron, for collapsing a disclosure.
    ChevronUp,
    /// A down chevron, for expanding a disclosure.
    ChevronDown,
    /// A vertical bar with a left chevron, for the first page.
    FirstPage,
    /// A vertical bar with a right chevron, for the last page.
    LastPage,
    /// A closed folder.
    Folder,
    /// An open folder.
    FolderOpen,
    /// A document sheet with ruled lines.
    File,
    /// A floppy disk, for saving.
    Save,
    /// Two stacked sheets, for copying.
    Copy,
    /// A printer, for printing.
    Print,
    /// A left-curving arrow, for undo.
    Undo,
    /// A right-curving arrow, for redo.
    Redo,
    /// A pair of scissors, for cutting.
    Cut,
    /// A clipboard, for pasting.
    Paste,
    /// A paperclip, for attaching a file.
    Attachment,
    /// Two chain links, for a hyperlink.
    Link,
    /// A filled circle with a tick, for a completed operation.
    Success,
    /// A question mark in a circle, for help.
    Help,
    /// A circle with a diagonal slash, for a disallowed action.
    Block,
    /// A clock face, for a scheduled time.
    Schedule,
    /// An hourglass, for a pending or in-progress state.
    Hourglass,
    /// A right-pointing triangle, for play.
    Play,
    /// Two vertical bars, for pause.
    Pause,
    /// A filled square, for stop.
    Stop,
    /// A triangle against a bar, for the next track.
    SkipNext,
    /// A speaker with sound waves, for unmuted audio.
    VolumeUp,
    /// A speaker with a cross, for muted audio.
    VolumeOff,
    /// A downward arrow over a bar, for sorting.
    Sort,
    /// Three ascending bars, for a chart.
    BarChart,
    /// A calendar page, for a date.
    Calendar,
    /// A grid of cells, for tabular data.
    Table,
    /// A speech bubble, for a conversation.
    Chat,
    /// A telephone handset, for a voice call.
    Call,
    /// A paper plane, for sending.
    Send,
    /// A bell with a cross, for muted notifications.
    NotificationsOff,
    /// A four-corner expand mark, for fullscreen.
    Fullscreen,
}

impl IconName {
    /// Returns the string representation of this icon name.
    ///
    /// The tokens are lower-case and underscore-separated (`"arrow_left"`), and are the
    /// exact spellings accepted by [`IconName::from_name`] and by the `icon` property. They
    /// are also the names used by [`Icon::set_icon`].
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Check => "check",
            Self::Cross => "cross",
            Self::ArrowLeft => "arrow_left",
            Self::ArrowRight => "arrow_right",
            Self::ArrowUp => "arrow_up",
            Self::ArrowDown => "arrow_down",
            Self::Star => "star",
            Self::Heart => "heart",
            Self::Settings => "settings",
            Self::Home => "home",
            Self::Search => "search",
            Self::Menu => "menu",
            Self::Close => "close",
            Self::Plus => "plus",
            Self::Minus => "minus",
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Error => "error",
            Self::User => "user",
            Self::Mail => "mail",
            Self::Bell => "bell",
            Self::Edit => "edit",
            Self::Trash => "trash",
            Self::Share => "share",
            Self::Refresh => "refresh",
            Self::More => "more",
            Self::Filter => "filter",
            Self::Lock => "lock",
            Self::Unlock => "unlock",
            Self::Download => "download",
            Self::Upload => "upload",
            Self::ChevronLeft => "chevron_left",
            Self::ChevronRight => "chevron_right",
            Self::ChevronUp => "chevron_up",
            Self::ChevronDown => "chevron_down",
            Self::FirstPage => "first_page",
            Self::LastPage => "last_page",
            Self::Folder => "folder",
            Self::FolderOpen => "folder_open",
            Self::File => "file",
            Self::Save => "save",
            Self::Copy => "copy",
            Self::Print => "print",
            Self::Undo => "undo",
            Self::Redo => "redo",
            Self::Cut => "cut",
            Self::Paste => "paste",
            Self::Attachment => "attachment",
            Self::Link => "link",
            Self::Success => "success",
            Self::Help => "help",
            Self::Block => "block",
            Self::Schedule => "schedule",
            Self::Hourglass => "hourglass",
            Self::Play => "play",
            Self::Pause => "pause",
            Self::Stop => "stop",
            Self::SkipNext => "skip_next",
            Self::VolumeUp => "volume_up",
            Self::VolumeOff => "volume_off",
            Self::Sort => "sort",
            Self::BarChart => "bar_chart",
            Self::Calendar => "calendar",
            Self::Table => "table",
            Self::Chat => "chat",
            Self::Call => "call",
            Self::Send => "send",
            Self::NotificationsOff => "notifications_off",
            Self::Fullscreen => "fullscreen",
        }
    }

    /// Parses a token back to its variant, or `None` when it is not one.
    ///
    /// The match is exact and case-sensitive: only the tokens produced by
    /// [`IconName::as_str`] are accepted, so `"ArrowLeft"` and `"arrow left"` both return
    /// `None`. Use [`Icon::set_icon`] when an unrecognised name should fall back to a
    /// placeholder rather than being rejected.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "check" => Some(Self::Check),
            "cross" => Some(Self::Cross),
            "arrow_left" => Some(Self::ArrowLeft),
            "arrow_right" => Some(Self::ArrowRight),
            "arrow_up" => Some(Self::ArrowUp),
            "arrow_down" => Some(Self::ArrowDown),
            "star" => Some(Self::Star),
            "heart" => Some(Self::Heart),
            "settings" => Some(Self::Settings),
            "home" => Some(Self::Home),
            "search" => Some(Self::Search),
            "menu" => Some(Self::Menu),
            "close" => Some(Self::Close),
            "plus" => Some(Self::Plus),
            "minus" => Some(Self::Minus),
            "info" => Some(Self::Info),
            "warning" => Some(Self::Warning),
            "error" => Some(Self::Error),
            "user" => Some(Self::User),
            "mail" => Some(Self::Mail),
            "bell" => Some(Self::Bell),
            "edit" => Some(Self::Edit),
            "trash" => Some(Self::Trash),
            "share" => Some(Self::Share),
            "refresh" => Some(Self::Refresh),
            "more" => Some(Self::More),
            "filter" => Some(Self::Filter),
            "lock" => Some(Self::Lock),
            "unlock" => Some(Self::Unlock),
            "download" => Some(Self::Download),
            "upload" => Some(Self::Upload),
            "chevron_left" => Some(Self::ChevronLeft),
            "chevron_right" => Some(Self::ChevronRight),
            "chevron_up" => Some(Self::ChevronUp),
            "chevron_down" => Some(Self::ChevronDown),
            "first_page" => Some(Self::FirstPage),
            "last_page" => Some(Self::LastPage),
            "folder" => Some(Self::Folder),
            "folder_open" => Some(Self::FolderOpen),
            "file" => Some(Self::File),
            "save" => Some(Self::Save),
            "copy" => Some(Self::Copy),
            "print" => Some(Self::Print),
            "undo" => Some(Self::Undo),
            "redo" => Some(Self::Redo),
            "cut" => Some(Self::Cut),
            "paste" => Some(Self::Paste),
            "attachment" => Some(Self::Attachment),
            "link" => Some(Self::Link),
            "success" => Some(Self::Success),
            "help" => Some(Self::Help),
            "block" => Some(Self::Block),
            "schedule" => Some(Self::Schedule),
            "hourglass" => Some(Self::Hourglass),
            "play" => Some(Self::Play),
            "pause" => Some(Self::Pause),
            "stop" => Some(Self::Stop),
            "skip_next" => Some(Self::SkipNext),
            "volume_up" => Some(Self::VolumeUp),
            "volume_off" => Some(Self::VolumeOff),
            "sort" => Some(Self::Sort),
            "bar_chart" => Some(Self::BarChart),
            "calendar" => Some(Self::Calendar),
            "table" => Some(Self::Table),
            "chat" => Some(Self::Chat),
            "call" => Some(Self::Call),
            "send" => Some(Self::Send),
            "notifications_off" => Some(Self::NotificationsOff),
            "fullscreen" => Some(Self::Fullscreen),
            _ => None,
        }
    }

    /// The canonical token of every variant, in declaration order.
    ///
    /// The table's own view of [`IconName::ALL`], so a test can compare the two lists rather
    /// than compare each against a third copy.
    pub fn all_tokens() -> [&'static str; 69] {
        let mut tokens = [""; 69];
        let mut index = 0;
        let mut variant_index = 0;
        while variant_index < Self::ALL.len() {
            tokens[index] = Self::ALL[variant_index].as_str();
            index += 1;
            variant_index += 1;
        }
        tokens
    }

    /// Every variant, in declaration order.
    ///
    /// Indexed directly by the enum's discriminant — `data()` and `data_opt()` rely on that
    /// order matching the generated tables, which the integrity tests assert by name.
    pub const ALL: [IconName; 69] = [
        Self::Check,
        Self::Cross,
        Self::ArrowLeft,
        Self::ArrowRight,
        Self::ArrowUp,
        Self::ArrowDown,
        Self::Star,
        Self::Heart,
        Self::Settings,
        Self::Home,
        Self::Search,
        Self::Menu,
        Self::Close,
        Self::Plus,
        Self::Minus,
        Self::Info,
        Self::Warning,
        Self::Error,
        Self::User,
        Self::Mail,
        Self::Bell,
        Self::Edit,
        Self::Trash,
        Self::Share,
        Self::Refresh,
        Self::More,
        Self::Filter,
        Self::Lock,
        Self::Unlock,
        Self::Download,
        Self::Upload,
        Self::ChevronLeft,
        Self::ChevronRight,
        Self::ChevronUp,
        Self::ChevronDown,
        Self::FirstPage,
        Self::LastPage,
        Self::Folder,
        Self::FolderOpen,
        Self::File,
        Self::Save,
        Self::Copy,
        Self::Print,
        Self::Undo,
        Self::Redo,
        Self::Cut,
        Self::Paste,
        Self::Attachment,
        Self::Link,
        Self::Success,
        Self::Help,
        Self::Block,
        Self::Schedule,
        Self::Hourglass,
        Self::Play,
        Self::Pause,
        Self::Stop,
        Self::SkipNext,
        Self::VolumeUp,
        Self::VolumeOff,
        Self::Sort,
        Self::BarChart,
        Self::Calendar,
        Self::Table,
        Self::Chat,
        Self::Call,
        Self::Send,
        Self::NotificationsOff,
        Self::Fullscreen,
    ];

    /// This icon's bundled outline data, indexed by declaration order.
    ///
    /// # Why the return is not an `Option`
    ///
    /// With the `icons` feature on, every variant has data — the generator refuses to emit a
    /// table with a gap — so a token with no outline is a compile error rather than a runtime
    /// placeholder. A `None` here would be a lie about a value that cannot be absent.
    #[cfg(feature = "icons")]
    pub fn data(self) -> IconData {
        use crate::widget::icon_data::ICON_DATA;
        // Indexed by the enum's declaration order, which is also `ICON_DATA`'s order: both
        // come from the same token list. The `debug_assert!` catches a reorder in a debug
        // build; `tests/icon_data_integrity_test.rs` checks the names in every build.
        let index = self as usize;
        debug_assert_eq!(
            ICON_DATA[index].name,
            self.as_str(),
            "ICON_DATA order must match IconName declaration order"
        );
        ICON_DATA[index]
    }

    /// [`Self::data`], available whether or not the feature is on.
    ///
    /// # Why two accessors rather than one returning `Option`
    ///
    /// The always-available spelling, so a draw path compiles in both states without a `cfg`
    /// at the call site. It is `Some` exactly when `icons` is on, and `None` otherwise, since
    /// without the feature there is no table to index.
    #[cfg(feature = "icons")]
    pub fn data_opt(self) -> Option<IconData> {
        Some(self.data())
    }

    /// [`Self::data`] for a build without the icon data: there is none, which is the honest
    /// answer rather than a fabricated entry.
    #[cfg(not(feature = "icons"))]
    pub fn data_opt(self) -> Option<IconData> {
        None
    }
}
