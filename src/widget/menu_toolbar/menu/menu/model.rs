// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The menu model: entries, mnemonic helpers, live state and the control's API.
//!
//! The rows, their selection and every mutation of the list live here. The popup's
//! layout, paint and hit testing are in `popup`; the trait implementations that connect
//! this model to the runtime are in `events`.

use crate::compat::{lock, Arc, Mutex, Rc, RefCell, Vec};
use crate::core::{ObjectId, Point, Rect};
use crate::signal::{GenericSignal, Signal1};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::companions::TooltipManager;
use crate::widget::menu_toolbar::action::Action;
use crate::widget::menu_toolbar::effects;
use crate::widget::menu_toolbar::popup_reveal::PopupReveal;
use crate::widget::menu_toolbar::tool_bar::child_hover_id;
use crate::widget::menu_toolbar::MAX_SUBMENU_DEPTH;
use crate::widget::{BaseWidget, IconSource, Widget, WidgetKind};

/// The mnemonic letter carried by a menu label.
///
/// A label marks its accelerator with `&`: `"&File"` is drawn as `File` and answered by
/// Alt+F. A doubled `&&` is a literal ampersand and marks nothing.
///
/// Returns `None` when the label carries no marker, which is the default for every
/// existing menu: parsing is metadata, so a label that never used `&` behaves exactly
/// as it did.
pub fn menu_mnemonic(text: &str) -> Option<char> {
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '&' {
            continue;
        }
        match chars.next() {
            Some('&') => continue,
            Some(next) => return Some(next),
            None => return None,
        }
    }
    None
}

/// Removes mnemonic markers from a label, keeping literal ampersands.
///
/// `"&&File"` becomes `"&File"`, `"&File"` becomes `"File"`, and a label without
/// markers is returned unchanged.
pub fn strip_menu_mnemonic(text: &str) -> String {
    let mut stripped = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '&' {
            stripped.push(ch);
            continue;
        }
        match chars.peek() {
            Some('&') => {
                chars.next();
                stripped.push('&');
            }
            Some(_) => { /* the marker itself is not drawn */ }
            None => stripped.push('&'),
        }
    }
    stripped
}

/// The first character a mnemonic label shows, skipping its `&` marker.
///
/// Type-ahead matches what the user reads, not the raw stored text: `"&File"`
/// starts with `F`, and `"&&File"` starts with the literal `&` it paints.
fn first_menu_label_char(text: &str) -> Option<char> {
    let mut chars = text.chars();
    match chars.next()? {
        '&' => match chars.next()? {
            '&' => Some('&'),
            first => Some(first),
        },
        first => Some(first),
    }
}

/// The ASCII letter or digit a key code names, for mnemonic accelerators.
///
/// Only letters and digits are mnemonics; every other key belongs to the menu's
/// navigation contract.
pub(super) fn mnemonic_letter(key: u32) -> Option<char> {
    match key {
        65..=90 => char::from_u32(key),
        48..=57 => char::from_u32(key),
        97..=122 => char::from_u32(key),
        _ => None,
    }
}

/// Width of the gap between two item columns, in logical pixels.
pub(super) const MENU_COLUMN_GAP: u32 = 8;

/// Where `Menu::open_at` places the popup relative to the anchor point.
///
/// A context menu has no host-owned frame to position it, so the direction is a
/// property of the menu rather than of the caller. [`PopupDirection::Down`] keeps the
/// historical lower-right placement; [`PopupDirection::Up`] anchors the popup's bottom
/// edge at the anchor; [`PopupDirection::Auto`] flips up when the popup would overflow
/// the viewport's bottom edge, which is what keeps a menu opened near the bottom of the
/// screen reachable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PopupDirection {
    /// Anchor the popup's top edge at the invocation point (historical behaviour).
    Down,
    /// Anchor the popup's bottom edge at the invocation point.
    Up,
    /// Use [`PopupDirection::Down`] when it fits and [`PopupDirection::Up`] otherwise.
    #[default]
    Auto,
}

/// A single item in a menu.
#[derive(Debug, Clone)]
pub struct MenuEntry {
    text: String,
    shortcut: String,
    checkable: bool,
    checked: bool,
    enabled: bool,
    separator: bool,
    has_submenu: bool,
    /// Optional icon shown in the row's indicator column.
    icon: Option<IconSource>,
    /// Child entries, when this row opens a nested panel.
    submenu: Option<Vec<MenuEntry>>,
    /// Whether the row is a section caption rather than a choice.
    section: bool,
    /// The registry command this row presents, when it was built from an [`ActionManager`].
    ///
    /// A plain row has no id: its meaning is its text. A row built from the registry keeps the
    /// id so activation can be routed to the command and state can be refreshed without
    /// re-creating the menu.
    action_id: Option<String>,
    /// The hover hint a host may show for this row.
    ///
    /// Stored, not painted: a tooltip is a popup owned by the host's tooltip pipeline, so
    /// keeping it on the row is what lets that pipeline answer without a second table.
    tooltip: String,
    /// The widget id of the [`Action`] this row presents, when it was built from one.
    ///
    /// Unlike [`Self::action_id`] (a registry string used by `ActionManager`), this is the
    /// live widget identity a host can use to look the action up and connect to its signals.
    action: Option<ObjectId>,
}
impl MenuEntry {
    /// Creates a visible, enabled entry carrying `text`.
    ///
    /// The shortcut, submenu marker and check marks start unset, and the entry is
    /// not a separator.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            shortcut: String::new(),
            checkable: false,
            checked: false,
            enabled: true,
            separator: false,
            has_submenu: false,
            icon: None,
            submenu: None,
            section: false,
            action_id: None,
            tooltip: String::new(),
            action: None,
        }
    }
    /// Creates a separator entry: an empty-text entry marked
    /// [`MenuEntry::is_separator`].
    ///
    /// A separator draws as a rule, is never the hovered entry, and is skipped
    /// when a press is resolved to an action.
    pub fn separator() -> Self {
        let mut m = Self::new("");
        m.set_separator(true);
        m
    }
    /// Sets the shortcut text and returns `self`, for chaining onto
    /// [`MenuEntry::new`].
    ///
    /// The string is **display text**, shown right-aligned in the entry: it is
    /// not parsed and not registered as an accelerator, so `"Ctrl+Shift+Z"` and
    /// `"⌘⇧Z"` are equally valid and equally inert. Nothing here chooses a
    /// notation for you; use `crate::format_shortcut` to render one that matches
    /// the host, as `WindowHandle::new_menu_item_with_shortcut` does.
    pub fn with_shortcut(mut self, shortcut: impl Into<String>) -> Self {
        self.shortcut = shortcut.into();
        self
    }

    // --- Accessors ---

    /// The entry's visible label. Empty for a separator.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Replaces the label. This is what is both drawn and, for an action, emitted
    /// from `Menu::triggered` when the entry is chosen.
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
    }

    /// The shortcut shown beside the entry, or `""` when none was set.
    pub fn shortcut(&self) -> &str {
        &self.shortcut
    }

    /// Replaces the shortcut display text. See [`MenuEntry::with_shortcut`] for
    /// what this string is and is not.
    pub fn set_shortcut(&mut self, shortcut: impl Into<String>) {
        self.shortcut = shortcut.into();
    }

    /// Whether the entry shows a check mark and is meant to toggle.
    ///
    /// Purely a drawing and policy flag on this type: choosing the entry emits
    /// the same signal either way, so the application is what flips
    /// [`MenuEntry::set_checked`] in response.
    pub fn is_checkable(&self) -> bool {
        self.checkable
    }

    /// Makes the entry checkable, or stops it being so. Turning it off leaves
    /// the checked value as it is.
    pub fn set_checkable(&mut self, checkable: bool) {
        self.checkable = checkable;
    }

    /// Whether the entry is currently marked as checked.
    pub fn is_checked(&self) -> bool {
        self.checked
    }

    /// Sets the checked mark directly.
    ///
    /// Note that `Menu::set_item_checked` ignores this on a non-checkable entry,
    /// whereas this method does not.
    pub fn set_checked(&mut self, checked: bool) {
        self.checked = checked;
    }

    /// Whether the entry can be chosen. A disabled entry draws greyed out and is
    /// not selectable — it is skipped when deciding what to hover and it emits
    /// nothing when pressed.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Enables or disables the entry. Disabling the entry currently under the
    /// cursor does not move the highlight, which stays where it is until the
    /// next pointer move.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Whether this entry is a divider rather than a choice.
    pub fn is_separator(&self) -> bool {
        self.separator
    }

    /// Marks the entry as a divider (`true`) or an ordinary entry (`false`).
    ///
    /// This only affects how the entry is treated, not what it holds — a
    /// separator can still carry text and a shortcut, which are then simply not
    /// drawn as an action row.
    pub fn set_separator(&mut self, separator: bool) {
        self.separator = separator;
    }

    /// Whether the entry is drawn with a submenu indicator.
    pub fn has_submenu(&self) -> bool {
        self.has_submenu
    }

    /// Shows or hides the submenu indicator.
    ///
    /// The flag alone marks the entry as a branch. Attaching children through
    /// [`MenuEntry::set_submenu`] is what makes this menu paint the nested panel itself;
    /// without children the flag stays a hint for a host that draws its own child menu.
    pub fn set_has_submenu(&mut self, has: bool) {
        self.has_submenu = has;
    }

    /// The icon painted in this row's indicator column, if any.
    pub fn icon(&self) -> Option<IconSource> {
        self.icon
    }

    /// Sets or clears the row icon. An icon takes the indicator column, so a
    /// checkable row with an icon shows the icon and no tick.
    pub fn set_icon(&mut self, icon: Option<IconSource>) {
        self.icon = icon;
    }

    /// Sets the row icon and returns `self`, for chaining onto [`MenuEntry::new`].
    pub fn with_icon(mut self, icon: IconSource) -> Self {
        self.icon = Some(icon);
        self
    }

    /// The nested entries this row opens, or `None` for a leaf.
    pub fn submenu(&self) -> Option<&[MenuEntry]> {
        self.submenu.as_deref()
    }

    /// Attaches nested entries, making this row a branch this menu can open itself.
    ///
    /// The panel is painted to the right of the parent row, and rows inside it may be branches
    /// of their own: a cascade opens up to [`MAX_SUBMENU_DEPTH`] panels. A branch on the deepest
    /// level is still reported through [`Menu::submenu_requested`] but cannot open another
    /// panel, which is the stated depth contract.
    pub fn set_submenu(&mut self, children: Option<Vec<MenuEntry>>) {
        self.submenu = children;
        self.has_submenu = self.submenu.as_deref().is_some_and(|c| !c.is_empty());
    }

    /// Sets nested entries and returns `self`.
    pub fn with_submenu(mut self, children: Vec<MenuEntry>) -> Self {
        self.set_submenu(Some(children));
        self
    }

    /// Creates a section caption: a non-selectable row that groups the entries below it.
    pub fn section(title: impl Into<String>) -> Self {
        let mut entry = Self::new(title);
        entry.section = true;
        entry
    }

    /// Whether this row is a section caption rather than a choice.
    pub fn is_section(&self) -> bool {
        self.section
    }

    /// Whether this row can be highlighted and chosen.
    ///
    /// Separators and section captions cannot; neither can a disabled entry.
    pub fn is_selectable(&self) -> bool {
        !self.separator && !self.section && self.enabled
    }

    /// The mnemonic letter this entry answers to, parsed from `&` in its label.
    pub fn mnemonic(&self) -> Option<char> {
        menu_mnemonic(&self.text)
    }

    /// The registry command this row presents, or `None` for a plain row.
    pub fn action_id(&self) -> Option<&str> {
        self.action_id.as_deref()
    }

    /// Links this row to a registry command without changing its presentation.
    pub fn set_action_id(&mut self, action_id: impl Into<String>) {
        self.action_id = Some(action_id.into());
    }

    /// The hover hint shown for this row, or `""` when none was set.
    pub fn tooltip(&self) -> &str {
        &self.tooltip
    }

    /// Replaces the hover hint. Metadata only: the menu does not paint a tooltip.
    pub fn set_tooltip(&mut self, tooltip: impl Into<String>) {
        self.tooltip = tooltip.into();
    }

    /// Sets the hover hint and returns `self`, for chaining onto [`MenuEntry::new`].
    pub fn with_tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.set_tooltip(tooltip);
        self
    }

    /// The widget id of the [`Action`] this row presents, if any.
    pub fn action(&self) -> Option<ObjectId> {
        self.action
    }

    /// Links this row to the widget id of an [`Action`].
    pub fn set_action(&mut self, action: Option<ObjectId>) {
        self.action = action;
    }
}

/// The declared shape of one menu row, as a record (BLUE26 B26.09).
///
/// This is the row shape of the `items` property, and the control owns both ends of it: the field
/// names are [`MenuEntry`]'s own accessors. `text` is required because [`MenuEntry::new`] requires
/// it; the rest are optional because a row that omits them describes the ordinary entry. Nested
/// `submenu` children have **no** representation: a record field cannot carry a nested record list
/// (`value_matches_kind` refuses it), so `has_submenu` declares the indicator while the panel's
/// rows are outside this channel.
pub const MENU_ITEM_SCHEMA: crate::widget::capability::RecordSchema =
    crate::widget::capability::RecordSchema {
        fields: &[
            crate::widget::capability::RecordField::required(
                "text",
                crate::widget::capability::PropertyValueKind::String,
            ),
            crate::widget::capability::RecordField::optional(
                "shortcut",
                crate::widget::capability::PropertyValueKind::String,
            ),
            crate::widget::capability::RecordField::optional(
                "checkable",
                crate::widget::capability::PropertyValueKind::Bool,
            ),
            crate::widget::capability::RecordField::optional(
                "checked",
                crate::widget::capability::PropertyValueKind::Bool,
            ),
            crate::widget::capability::RecordField::optional(
                "enabled",
                crate::widget::capability::PropertyValueKind::Bool,
            ),
            crate::widget::capability::RecordField::optional(
                "separator",
                crate::widget::capability::PropertyValueKind::Bool,
            ),
            crate::widget::capability::RecordField::optional(
                "has_submenu",
                crate::widget::capability::PropertyValueKind::Bool,
            ),
            crate::widget::capability::RecordField::optional(
                "section",
                crate::widget::capability::PropertyValueKind::Bool,
            ),
            crate::widget::capability::RecordField::optional(
                "tooltip",
                crate::widget::capability::PropertyValueKind::String,
            ),
            crate::widget::capability::RecordField::optional(
                "action_id",
                crate::widget::capability::PropertyValueKind::String,
            ),
            crate::widget::capability::RecordField::optional(
                "icon_source",
                crate::widget::capability::PropertyValueKind::IconSpec,
            ),
        ],
        // A plain row's meaning is its text; there is no separate id. Two rows may share a label,
        // so the identity field is `text` with the uniqueness rule off.
        id_field: "text",
        ids_unique: false,
        ids_required: false,
    };

/// One row of [`MENU_ITEM_SCHEMA`]; refuses a source with no document form.
pub(super) fn menu_entry_record(
    entry: &MenuEntry,
) -> Result<crate::widget::capability::RecordValue, CapabilityAccessError> {
    let mut record = crate::widget::capability::RecordValue::new()
        .with("text", CapabilityValue::String(entry.text.clone()))
        .with("shortcut", CapabilityValue::String(entry.shortcut.clone()))
        .with("checkable", CapabilityValue::Bool(entry.checkable))
        .with("checked", CapabilityValue::Bool(entry.checked))
        .with("enabled", CapabilityValue::Bool(entry.enabled))
        .with("separator", CapabilityValue::Bool(entry.separator))
        .with("has_submenu", CapabilityValue::Bool(entry.has_submenu))
        .with("section", CapabilityValue::Bool(entry.section))
        .with("tooltip", CapabilityValue::String(entry.tooltip.clone()));
    if let Some(action_id) = &entry.action_id {
        record = record.with("action_id", CapabilityValue::String(action_id.clone()));
    }
    match entry.icon {
        None => {}
        Some(IconSource::Builtin(name)) => {
            record = record.with(
                "icon_source",
                CapabilityValue::IconSpec(crate::widget::icon_source::IconSpec::Builtin(name)),
            );
        }
        // A borrowed static geometry has no document form, so the read refuses rather than
        // reporting the row as icon-less.
        Some(IconSource::StaticGeometry(_)) => {
            return Err(CapabilityAccessError::IconSourceNotPersistable)
        }
    }
    Ok(record)
}

/// Rebuilds one [`MenuEntry`] from a row of [`MENU_ITEM_SCHEMA`].
pub(super) fn menu_entry_from_record(
    record: &crate::widget::capability::RecordValue,
) -> Result<MenuEntry, CapabilityAccessError> {
    let text = match record.get("text") {
        Some(CapabilityValue::String(text)) => text.clone(),
        _ => return Err(CapabilityAccessError::TypeMismatch),
    };
    let string_field = |name: &str| match record.get(name) {
        None | Some(CapabilityValue::Null) => Ok(String::new()),
        Some(CapabilityValue::String(value)) => Ok(value.clone()),
        Some(_) => Err(CapabilityAccessError::TypeMismatch),
    };
    let bool_field = |name: &str, default: bool| match record.get(name) {
        None | Some(CapabilityValue::Null) => Ok(default),
        Some(CapabilityValue::Bool(flag)) => Ok(*flag),
        Some(_) => Err(CapabilityAccessError::TypeMismatch),
    };
    let icon_source = match record.get("icon_source") {
        None | Some(CapabilityValue::Null) => None,
        Some(CapabilityValue::IconSpec(crate::widget::icon_source::IconSpec::Builtin(name))) => {
            Some(IconSource::Builtin(*name))
        }
        // A resource descriptor needs the host's resolver, which a property write does not have.
        Some(CapabilityValue::IconSpec(crate::widget::icon_source::IconSpec::Resource {
            ..
        })) => return Err(CapabilityAccessError::ResourceResolutionRequired),
        Some(_) => return Err(CapabilityAccessError::TypeMismatch),
    };

    let mut entry = MenuEntry::new(text);
    entry.shortcut = string_field("shortcut")?;
    entry.checkable = bool_field("checkable", false)?;
    entry.checked = bool_field("checked", false)?;
    entry.enabled = bool_field("enabled", true)?;
    entry.separator = bool_field("separator", false)?;
    entry.has_submenu = bool_field("has_submenu", false)?;
    entry.section = bool_field("section", false)?;
    entry.tooltip = string_field("tooltip")?;
    let action_id = string_field("action_id")?;
    if !action_id.is_empty() {
        entry.set_action_id(action_id);
    }
    entry.set_icon(icon_source);
    Ok(entry)
}

// A hover source adapter is not needed: [`MenuEntry`] stores its hint directly.

/// The facts [`Menu::popup_height`] is a pure function of.
///
/// Opening a menu used to re-derive the same height on every call while the reveal was in
/// flight. The key is the list revision plus everything the height arithmetic reads, so a
/// cached value cannot survive a change that moves a row or changes the visible window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct PopupMetricsKey {
    pub(super) version: u64,
    pub(super) width: u32,
    pub(super) columns: u8,
    pub(super) tearoff: bool,
    /// The configured visible-item cap (part of `visible_range`).
    pub(super) capacity: usize,
    /// The resolved font size's bit pattern; the heading measurement is a function of it.
    pub(super) font_bits: u32,
}

/// One row's mnemonic presentation: the stripped label and the measured underline span.
///
/// Stripping `&` and measuring the prefix are both functions of the label and the font, so
/// they are derived once per list/font revision instead of on every painted frame (G-P06).
pub(super) struct MnemonicRow {
    /// The label with its marker removed.
    pub(super) text: String,
    /// The mnemonic letter and the `(prefix width, glyph width)` of its underline.
    pub(super) underline: Option<(char, u32, u32)>,
}

/// The facts [`Menu::item_bands`] is a pure function of.
///
/// A version counter alongside the geometry is what makes the cache cheap to validate: every
/// entry mutation bumps the counter, so a cached run cannot survive a change that moves a row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ItemBandsKey {
    pub(super) version: u64,
    pub(super) width: u32,
    pub(super) scroll_offset: usize,
    pub(super) tearoff: bool,
    pub(super) columns: u8,
}

/// Builds the menu row that presents `action`.
///
/// Presentation is copied rather than borrowed because a menu owns its rows; the live link is
/// the action's widget id in [`MenuEntry::action`] plus the command id used by
/// [`Menu::action_triggered`].
fn menu_entry_from_action(action: &Action) -> MenuEntry {
    let mut entry = MenuEntry::new(action.text());
    if let Some(id) = action.command_id() {
        entry.set_action_id(id);
    }
    if !action.shortcut().is_empty() {
        entry.set_shortcut(action.shortcut());
    }
    entry.set_checkable(action.is_checkable());
    if action.is_checkable() {
        entry.set_checked(action.is_checked());
    }
    entry.set_enabled(action.cmd().is_enabled());
    entry.set_icon(action.icon_source());
    entry.set_tooltip(action.status_tip());
    entry.set_action(Some(action.base().id()));
    entry
}

/// One open nested panel of a [`Menu`].
///
/// Only the row that opened the panel and that panel's own highlight are stored: the rows
/// themselves stay in the entry tree, so a check mark changed while the panel is open is
/// visible without a second copy to keep in sync. The path is a stack because four open
/// levels are one control state, not four mutually exclusive `Option` fields.
#[derive(Clone, Copy, Debug)]
pub(super) struct SubmenuLevel {
    /// The row in the parent level that opened this panel.
    pub(super) row: usize,
    /// The highlighted row inside this panel.
    pub(super) hovered: Option<usize>,
}
/// Menu widget.
///
/// # Two uses
///
/// The same type serves a menu-bar drop-down and a **context menu** (the menu a
/// right-click opens). `WidgetKind::ContextMenu` resolves to this type, so the
/// behaviour a context menu needs lives here rather than in a second, parallel
/// menu implementation (principle #54). That resolution is recorded in one place —
/// the `ContextMenu` arm of `capability::alias_factory_name`, paired with the
/// `context_menu` -> `menu` row in `capability::ALIAS_OF` (G-305) — so this comment,
/// that arm and the `WidgetKind::ContextMenu` variant comment form one anchor. The
/// context-menu behaviour it serves:
///
/// * [`Self::open_at`] places the popup at a pointer position, clamping it so the
///   menu stays on screen;
/// * a press **outside** the popup dismisses it, which is what makes a context
///   menu feel like a context menu rather than a panel that will not go away.
///
/// # Why dismissal is not keyed on the button
///
/// A popup is dismissed by a press anywhere outside it, whichever button the user
/// pressed. Handling only the secondary button would leave the menu open when the
/// user left-clicks away, which is the single most common way people dismiss one.
pub struct Menu {
    pub(super) base: BaseWidget,
    pub(super) title: String,
    pub(super) items: Vec<MenuEntry>,
    /// An optional flat panel mirroring the top-level rows (gap 5).
    panel: Option<crate::widget::companions::MenuPanel>,
    pub(super) hovered_index: Option<usize>,
    /// Screen position the popup was opened at, for diagnostics and tests.
    pub(super) invoker_position: Option<Point>,
    /// The area the popup and its cascading panels must stay inside.
    ///
    /// [`Self::open_at`] receives the host surface rectangle, and nested panels
    /// are clamped and flipped against the same rectangle. `None` keeps the
    /// historical right-hanging layout, which is what a menu opened by a path
    /// that supplied no viewport still gets (M-006).
    pub(super) viewport: Option<Rect>,
    /// Emitted with the text of the entry that was chosen.
    ///
    /// Fires once per successful press on an enabled, non-separator entry; the
    /// menu hides immediately afterwards.
    pub triggered: Signal1<String>,
    /// Emitted with the index of the entry that was chosen, alongside
    /// [`Self::triggered`]. Use this when two entries share a label.
    pub triggered_index: Signal1<usize>,
    /// Emitted with the **action id** of a row built from an `ActionManager`.
    ///
    /// This is the consumer path the action registry was missing: a menu item carries the id it
    /// was resolved from, so a host can route the activation to `ActionManager::trigger_action`
    /// without parsing the label. A plain row emits nothing here.
    pub action_triggered: Signal1<String>,
    /// Emitted just before the menu is shown, from [`Self::open_at`].
    pub about_to_show: GenericSignal,
    /// Emitted with the label of an entry whose **submenu** the user asked to open.
    ///
    /// # Why this is separate from `triggered` (BLUE22 · G-3)
    ///
    /// An entry drawn with a submenu indicator is a **branch**: choosing it means "show me the
    /// next level", not "run this command". This menu paints the child panel itself, up to
    /// [`MAX_SUBMENU_DEPTH`] levels, and still reports every opened branch through this signal,
    /// so a host can mirror or extend the tree without mistaking the gesture for a committed
    /// choice. Before this signal existed, both pointer paths emitted `triggered` and hid the
    /// popup for a submenu parent, so a click on `Cut >` fired a `Cut` action and closed the
    /// popup; a host that ignores the signal now gets nothing it could mistake for a commit.
    ///
    /// The pointer paths use this exactly as the arrow-key path does: it is the one way this
    /// control says "a branch was opened".
    pub submenu_requested: Signal1<String>,
    /// Emitted just before the menu is hidden, from [`Self::hide`].
    pub about_to_hide: GenericSignal,
    /// The writing direction the menu's *horizontal* arrow keys follow.
    ///
    /// # Why a vertical list still needs this
    ///
    /// A menu of entries runs top to bottom, which no script reverses — `Up`/`Down` walk the list in
    /// every locale. But a menu is also the child of a menu **bar**, and the left/right pair is what
    /// steps between a bar's drop-downs and into a submenu. Those two directions *are* a line of text
    /// direction, so in an Arabic or Hebrew interface `Right` means "the previous entry" and `Left`
    /// means "the next", the opposite of the Latin reading.
    ///
    /// The field therefore governs the horizontal pair only; [`Self::move_hovered_vertically`] ignores
    /// it deliberately.
    ///
    /// Defaults to left-to-right, so a menu that never asks behaves exactly as it did.
    direction: crate::core::TextDirection,
    /// The popup body's open/close reveal, so the entries grow out of the heading rather than
    /// appearing on the frame the menu opens.
    ///
    /// # Why the reveal is separate from `is_visible`
    ///
    /// `is_visible` is the *authority*: it is what hit testing, `about_to_show`/`about_to_hide`
    /// and the `"visible"` property answer with, and it flips on the frame the menu opens. A
    /// `bool` cannot express a popup mid-open, and the draw path needs the fraction — so the flag
    /// stays the truth and this says how far the transition has got. Closing is the same pair in
    /// reverse, which is why `hide` does not empty the menu: a panel that lost its entries as it
    /// closed would animate nothing.
    pub(super) reveal: PopupReveal,
    /// Whether the popup paints its title heading above the entry rows.
    ///
    /// A menu opened **from a menu bar** must not repeat its title: the bar already shows it
    /// two pixels above the popup, and a heading there makes the drop-down start with a copy of
    /// the label the user just clicked. A standalone context menu has no such label above it, so
    /// it keeps the heading. The flag is data rather than a second `Menu` type (principle #54).
    show_heading: bool,
    /// Index of the first entry the popup paints when the configured cap is smaller than
    /// the list.
    ///
    /// Only `menu_config` sets a cap; with the policy off this stays `0` and every entry
    /// is painted, exactly as before the wiring.
    pub(super) scroll_offset: usize,
    /// Cached heading line height, keyed by the font size's bit pattern.
    ///
    /// The heading is one shaped measurement per frame; a menu that is open while an
    /// animation runs paid it on every frame of the reveal. The cache is now unconditional:
    /// selection of the font never changes inside one menu, so gating it on the effect
    /// policy only made the default path pay the measurement.
    pub(super) heading_line_cache: core::cell::Cell<Option<(u32, u32)>>,
    /// The number of columns the popup arranges its entries into.
    ///
    /// `1` (the default) is the historical single-column list. A value above one splits the
    /// popup's inner width into equal columns and fills them row-major, which is what lets a
    /// menu of short entries stop being needlessly tall.
    pub(super) column_count: u8,
    /// Where [`Self::open_at`] anchors the popup. See [`PopupDirection`].
    pub(super) popup_direction: PopupDirection,
    /// Bumped by every entry mutation; [`ItemBandsKey`] compares it to decide whether the
    /// cached row boxes are still valid.
    pub(super) items_version: u64,
    /// The cached row boxes, keyed by [`ItemBandsKey`].
    pub(super) item_bands_cache: RefCell<Option<(ItemBandsKey, Rc<Vec<Rect>>)>>,
    /// The cached popup height, keyed by [`PopupMetricsKey`].
    pub(super) popup_metrics_cache: core::cell::Cell<Option<(PopupMetricsKey, f32)>>,
    /// The cached mnemonic rows, keyed by `(items revision, font size bits)`.
    pub(super) mnemonic_cache: RefCell<Option<(u64, u32, Rc<Vec<MnemonicRow>>)>>,
    /// The cached trailing-column width, keyed by `(items revision, font size bits)`.
    ///
    /// `draw` reads the trailing column once per row and `label_box` reads it again,
    /// so measuring every shortcut per row was O(n²) in the entry count (M-003).
    /// The width is a pure function of the list and the font, and every list mutation
    /// bumps `items_version`, so one pass fills it and later rows hit the cell.
    pub(super) trailing_width_cache: core::cell::Cell<Option<(u64, u32, u32)>>,
    /// The branch row the pointer is resting on, awaiting the hover-open delay.
    pub(super) hover_open_pending: Option<(usize, usize)>,
    /// How long the pointer has rested on [`Self::hover_open_pending`], in milliseconds.
    pub(super) hover_open_elapsed_ms: u32,
    /// The open nested panels, deepest last. One step per level, up to
    /// [`MAX_SUBMENU_DEPTH`]; level 0 is the menu's own list.
    pub(super) submenu_path: Vec<SubmenuLevel>,
    /// Whether the popup paints a tear-off strip above its rows.
    pub(super) tearoff: bool,
    /// Emitted when the tear-off strip is pressed.
    ///
    /// The menu does not remove itself: a host that supports a torn-off copy connects
    /// this and creates the detached window. A host that ignores it simply sees the
    /// press do nothing, which is the honest failure rather than a silent dismissal.
    pub tearoff_requested: GenericSignal,
    /// Whether mnemonic markers are stripped and underlined.
    ///
    /// Off by default: a menu that never asks paints its labels exactly as before, so
    /// wiring the accelerator parser changed no pixels.
    pub(super) show_mnemonics: bool,
    /// Live state cells for rows built from [`crate::action::ActionManager`].
    ///
    /// Each cell is subscribed to the command's own `toggled` / `enabled_changed` signals, so a
    /// change made through the registry is visible without the menu polling it. The row keeps its
    /// `action_id`; this is the state half of that link.
    live_actions: Vec<(String, Arc<Mutex<crate::action::ActionLiveState>>)>,
    /// The host-side hover/delay/show pipeline for row hints.
    pub(super) tooltips: TooltipManager,
    /// The derived id currently registered as hovered, if any.
    pub(super) tooltip_source: Option<ObjectId>,
    /// Monotonic clock fed to the tooltip pipeline from [`Widget::tick`].
    pub(super) tooltip_now_ms: u64,
}
impl Menu {
    /// Creates a menu titled `title`, initially **hidden**.
    ///
    /// A menu is a popup, so it does not paint until something opens it — call
    /// [`Menu::open_at`] for a context menu, or let a menu bar open it as a
    /// drop-down. This differs from most widgets, which start visible.
    ///
    /// The menu starts with no items.
    pub fn new(title: impl Into<String>, geometry: Rect) -> Self {
        let mut menu = Self {
            base: BaseWidget::new(WidgetKind::Menu, geometry, "Menu"),
            title: title.into(),
            items: Vec::new(),
            panel: None,
            hovered_index: None,
            invoker_position: None,
            viewport: None,
            triggered: Signal1::new(),
            triggered_index: Signal1::new(),
            action_triggered: Signal1::new(),
            about_to_show: GenericSignal::new(),
            submenu_requested: Signal1::new(),
            about_to_hide: GenericSignal::new(),
            direction: crate::core::TextDirection::default(),
            reveal: PopupReveal::new(false),
            show_heading: true,
            scroll_offset: 0,
            heading_line_cache: core::cell::Cell::new(None),
            column_count: 1,
            popup_direction: PopupDirection::Auto,
            items_version: 0,
            item_bands_cache: RefCell::new(None),
            popup_metrics_cache: core::cell::Cell::new(None),
            mnemonic_cache: RefCell::new(None),
            trailing_width_cache: core::cell::Cell::new(None),
            hover_open_pending: None,
            hover_open_elapsed_ms: 0,
            submenu_path: Vec::new(),
            tearoff: false,
            tearoff_requested: GenericSignal::new(),
            show_mnemonics: false,
            live_actions: Vec::new(),
            tooltips: TooltipManager::new(geometry),
            tooltip_source: None,
            tooltip_now_ms: 0,
        };
        // A menu is a popup, so it starts hidden. `BaseWidget` defaults to visible,
        // which is right for a control that owns part of the surface but wrong for
        // one that appears on demand: before this, a freshly created `Menu` (and so
        // a `WidgetKind::ContextMenu`) painted itself immediately at its stored
        // geometry, i.e. as a permanently open drop-down with no way to have created
        // it closed.
        //
        // Callers that want a menu on screen call `open_at`, or any of the menu
        // bar's own open paths, so hiding here does not remove a capability.
        menu.base.hide();
        menu
    }

    /// The pointer position this menu was opened at, if it was opened via
    /// [`Self::open_at`].
    pub fn invoker_position(&self) -> Option<Point> {
        self.invoker_position
    }

    /// The menu's title, drawn as its heading.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Returns the writing direction the menu's horizontal arrow keys follow.
    pub fn direction(&self) -> crate::core::TextDirection {
        self.direction
    }

    /// Sets the writing direction the menu's horizontal arrow keys follow, and repaints.
    ///
    /// `Left`/`Right` step along a line of reading order, so in a right-to-left interface `Right`
    /// moves to the **previous** entry and `Left` to the next. `Up`/`Down` are unaffected: a list of
    /// entries runs down in every locale. See the field for the reasoning.
    pub fn set_direction(&mut self, direction: crate::core::TextDirection) {
        if self.direction != direction {
            self.direction = direction;
            self.base.request_redraw();
        }
    }

    /// Where [`Self::open_at`] anchors the popup. Defaults to [`PopupDirection::Auto`].
    pub fn popup_direction(&self) -> PopupDirection {
        self.popup_direction
    }

    /// Selects the popup's anchor edge. The geometry is recomputed by the next
    /// [`Self::open_at`] call, so a menu already on screen is not moved.
    pub fn set_popup_direction(&mut self, direction: PopupDirection) {
        if self.popup_direction != direction {
            self.popup_direction = direction;
            self.base.request_redraw();
        }
    }

    /// The number of columns the popup arranges its entries into. Defaults to `1`.
    pub fn columns(&self) -> u8 {
        self.column_count
    }

    /// Arranges the entries into `columns` equal-width columns and repaints.
    ///
    /// A count below one is raised to one, which is the historical single-column list. A
    /// single-column menu lays rows out exactly as before, separator height included; a
    /// multi-column menu fills row-major so the reading order still matches the entry list.
    pub fn set_column_count(&mut self, columns: u8) {
        let columns = columns.max(1);
        if self.column_count == columns {
            return;
        }
        self.column_count = columns;
        self.base.request_redraw();
    }

    /// Programmatically highlights the entry at `index`.
    ///
    /// This is the public counterpart of the highlight the pointer and the arrow keys move:
    /// without it a host that drives a menu from its own model (a command palette, a test, a
    /// remote-control surface) could not put the selection on the row it considers active.
    /// Separators, section captions and disabled entries are not selectable, so pointing at one
    /// leaves the highlight untouched and returns `false`.
    pub fn set_active_action(&mut self, index: usize) -> bool {
        let selectable = self.items.get(index).is_some_and(MenuEntry::is_selectable);
        if !selectable {
            return false;
        }
        self.hovered_index = Some(index);
        self.ensure_hovered_visible();
        self.base.request_redraw();
        true
    }

    /// Moves the highlight by `delta` entries in reading order, skipping anything that cannot be
    /// chosen, and reports whether it landed on an enabled entry.
    ///
    /// # Why the highlight wraps and skips
    ///
    /// A menu whose highlight ran off the end and stopped would make every entry past the first
    /// ↓ unreachable without a mouse — the wrap is what makes a keyboard-only menu possible. A
    /// separator has no label to select and a disabled entry cannot be chosen, so both are stepped
    /// over; stopping *on* one would leave `Enter` doing nothing with no visible explanation.
    ///
    /// # Why the entry count, not the index count
    ///
    /// The caller passes a delta already expressed in reading order, so this function needs no
    /// knowledge of the direction. Keeping the conversion at the key handler means the vertical and
    /// horizontal steps share one implementation and cannot disagree about what "next" means.
    fn move_hovered_vertically(&mut self, delta: i32) -> bool {
        let count = self.items.len();
        if count == 0 || delta == 0 {
            return false;
        }
        // The starting point is the current highlight, or the list edge when nothing is highlighted
        // and the caller is stepping *toward* the list — so a first `Down` enters at the top and a
        // first `Up` at the bottom.
        let start = match self.hovered_index {
            Some(index) => index as i32,
            None if delta > 0 => -1,
            None => count as i32,
        };
        let mut index = start;
        for _ in 0..count {
            index += delta;
            if index < 0 {
                index = count as i32 - 1;
            } else if index >= count as i32 {
                index = 0;
            }
            let candidate = &self.items[index as usize];
            if candidate.is_selectable() {
                self.hovered_index = Some(index as usize);
                self.ensure_hovered_visible();
                self.base.request_redraw();
                return true;
            }
        }
        // Every entry is a separator or disabled: nothing to select, and the highlight must not be
        // invented. Returning `false` lets the caller decide whether that is an error.
        false
    }

    /// Moves the highlight by one entry in the direction a horizontal arrow key names.
    ///
    /// `ArrowRight` steps toward the end of the line in left-to-right and toward its beginning in
    /// right-to-left, which is the whole content of the direction field as it applies here.
    pub(super) fn move_hovered_horizontally(&mut self, arrow_right: bool) -> bool {
        // "Right" is +1 in left-to-right; the direction's step conversion is exactly the negation
        // rule, so a third direction would extend one place instead of two.
        let step = if arrow_right { 1 } else { -1 };
        let delta = self.direction.begin_step_to_left_step(step);
        self.move_hovered_vertically(delta)
    }

    /// Triggers the entry the highlight is on, if it can still be chosen.
    pub(super) fn activate_hovered(&mut self) -> bool {
        self.activate_level_hovered(0)
    }

    /// Triggers the highlighted row of `level`, if it can still be chosen.
    ///
    /// An entry that owns a submenu is a **branch, not a leaf**: activating it means "open
    /// the next level", never "choose this". The old code emitted `triggered` and hid the
    /// menu, so a click on `Cut >` fired the `Cut` action and closed the popup — the submenu
    /// was unreachable by pointer at all, and the user got a command they did not ask for.
    ///
    /// The branch is reported through `submenu_requested` — the same signal the arrow keys
    /// use — so a host has one thing to listen for and still receives nothing it could
    /// mistake for a committed choice.
    pub(super) fn activate_level_hovered(&mut self, level: usize) -> bool {
        let Some(index) = self.level_hovered(level) else {
            return false;
        };
        let Some(entry) = self.level_entries(level).and_then(|entries| entries.get(index)) else {
            return false;
        };
        if !entry.is_selectable() {
            return false;
        }
        let text = entry.text().to_string();
        let action_id = entry.action_id().map(String::from);
        if entry.has_submenu() {
            self.open_submenu_at(level, index);
            self.submenu_requested.emit(text);
            self.base.request_redraw();
            return true;
        }
        self.triggered.emit(text);
        self.triggered_index.emit(index);
        if let Some(action_id) = action_id {
            self.action_triggered.emit(action_id);
        }
        self.hide();
        true
    }
    /// Replaces the title, which the menu draws as its heading.
    ///
    /// This has nothing to do with the title a menu-bar entry shows — that comes
    /// from the menu-bar API, not from this type.
    pub fn set_title(&mut self, title: String) {
        if self.title == title {
            return;
        }
        self.title = title;
        self.base.request_redraw();
    }
    /// The entries, in draw order, including separators.
    pub fn items(&self) -> &[MenuEntry] {
        &self.items
    }
    /// The index of the entry currently highlighted, if any.
    ///
    /// Set by pointer movement and by [`Menu::open_at`], which highlights the
    /// first actionable entry. Never points at a separator when set by
    /// `open_at`; a pointer move onto a separator clears it.
    pub fn hovered_index(&self) -> Option<usize> {
        self.hovered_index
    }

    /// Registers the hovered row's hint with the tooltip pipeline.
    ///
    /// The hint is read straight off the row rather than through a property adapter: the pointer
    /// move path runs on every frame the pointer travels, and routing it through
    /// `WidgetProperties::get` made a hot path pay a string clone plus a dynamic name lookup (G-P06).
    pub(super) fn sync_hover_tooltip(&mut self) {
        let Some(index) = self.hovered_index else {
            if self.tooltip_source.take().is_some() {
                self.tooltips.note_leave();
            }
            return;
        };
        let Some(entry) = self.items.get(index) else {
            if self.tooltip_source.take().is_some() {
                self.tooltips.note_leave();
            }
            return;
        };
        let text = (!entry.tooltip.is_empty()).then_some(entry.tooltip.as_str());
        match text {
            Some(text) => {
                let id = child_hover_id(self.base.id(), index);
                // Compare against the registration before copying: a pointer move within
                // one row used to clone the hint on every event (M-004). The copy now
                // happens only when the text actually changed.
                if self.tooltips.text_for(id) != Some(text) {
                    self.tooltips.register(id, text);
                }
                self.tooltips.note_hover(id);
                self.tooltip_source = Some(id);
            }
            None => {
                if self.tooltip_source.take().is_some() {
                    self.tooltips.note_leave();
                }
            }
        }
    }

    /// Whether the tooltip pipeline still owes frames.
    pub(super) fn tooltip_owes_frames(&self) -> bool {
        self.tooltip_source.is_some() || self.tooltips.tooltip().is_some()
    }

    /// Appends a pre-built entry.
    ///
    /// No repaint is requested, so a visible menu does not redraw until
    /// something else invalidates it.
    pub fn add_item(&mut self, item: MenuEntry) {
        self.items.push(item);
        self.touch_items();
        self.clamp_scroll();
    }

    /// Appends a row resolved from the action registry.
    ///
    /// `text`, `enabled`, `checked`, `checkable` and the shortcut label all come from the
    /// registered command, so the menu is a view of the registry rather than a second declaration
    /// of it. The row keeps the action id; activating it emits [`Self::action_triggered`].
    /// Returns `None` when the id is unknown, which is the same answer
    /// `ActionManager::action_definition` gives instead of inventing an empty row.
    pub fn add_action_from_manager(
        &mut self,
        manager: &mut crate::action::ActionManager,
        action_id: &str,
    ) -> Option<usize> {
        let definition = manager.action_definition(action_id)?;
        // Resolving an item and recording the host binding are one step on purpose: a row in this
        // menu that was *not* registered as a host would make `bindings_for_host` report less than
        // the UI shows.
        if !manager.bind_action_to_menu(definition.id.clone(), self.base.id()) {
            return None;
        }
        let mut entry = MenuEntry::new(definition.text);
        entry.set_action_id(definition.id.clone());
        entry.set_enabled(definition.enabled);
        entry.set_checkable(definition.checkable);
        if definition.checkable {
            entry.set_checked(definition.checked);
        }
        if !definition.shortcut.is_empty() {
            entry.set_shortcut(definition.shortcut);
        }
        let index = self.items.len();
        self.items.push(entry);
        if let Some(state) = manager.watch_action(&definition.id) {
            self.live_actions.push((definition.id.clone(), state));
        }
        self.touch_items();
        self.clamp_scroll();
        self.base.request_redraw();
        Some(index)
    }

    /// The live state cell for `action_id`, if this menu has a row linked to it.
    pub fn live_action_state(&self, action_id: &str) -> Option<crate::action::ActionLiveState> {
        self.live_actions
            .iter()
            .find(|(id, _)| id == action_id)
            .map(|(_, state)| lock(state).clone())
    }

    /// Re-reads every linked row from `manager`.
    ///
    /// The registry is the source of truth; a host calls this after changing an action's state
    /// rather than mutating the same fact twice. Rows without an action id are left alone.
    pub fn refresh_action_states(&mut self, manager: &crate::action::ActionManager) {
        let mut changed = false;
        for item in &mut self.items {
            let Some(action_id) = item.action_id().map(String::from) else {
                continue;
            };
            // Prefer the live cell: it is the snapshot the action's own signals updated, so a
            // state change made through the registry is observed without a second lookup. Fall
            // back to the registry for a row that was linked before this cell existed.
            let live = self
                .live_actions
                .iter()
                .find(|(id, _)| id == &action_id)
                .map(|(_, state)| lock(state).clone());
            let Some(definition) = manager.action_definition(&action_id) else {
                continue;
            };
            let (text, enabled, checkable, checked) = match live {
                Some(state) => (state.text, state.enabled, state.checkable, state.checked),
                None => {
                    (definition.text, definition.enabled, definition.checkable, definition.checked)
                }
            };
            if item.text() != text {
                item.set_text(text);
                changed = true;
            }
            if item.is_enabled() != enabled {
                item.set_enabled(enabled);
                changed = true;
            }
            if item.is_checkable() != checkable {
                item.set_checkable(checkable);
                changed = true;
            }
            if checkable && item.is_checked() != checked {
                item.set_checked(checked);
                changed = true;
            }
            if item.shortcut() != definition.shortcut {
                item.set_shortcut(definition.shortcut);
                changed = true;
            }
        }
        if changed {
            self.base.request_redraw();
        }
    }

    /// Registers every linked row's shortcut in the global registry.
    ///
    /// This is the "native menu shortcut becomes a registry binding" step: a row's displayed
    /// accelerator is parsed once and inserted into `ShortcutManager`, so conflict detection
    /// and dispatch see it like any other chord. A glyph spelling that
    /// [`crate::shortcut::Shortcut::from_string`] cannot parse is skipped rather than guessed at.
    /// Returns the number of bindings registered.
    pub fn register_shortcuts_with(&self, manager: &mut crate::shortcut::ShortcutManager) -> usize {
        let mut registered = 0usize;
        for item in &self.items {
            let Some(action_id) = item.action_id() else {
                continue;
            };
            let Some(binding) = crate::shortcut::Shortcut::from_string(item.shortcut()) else {
                continue;
            };
            if manager.register(String::from(action_id), binding, item.text()) {
                registered += 1;
            }
        }
        registered
    }
    /// Appends a separator entry.
    pub fn add_separator(&mut self) {
        self.items.push(MenuEntry::separator());
        self.touch_items();
        self.clamp_scroll();
    }
    /// Appends a plain action labeled `text` and returns its index.
    ///
    /// The returned index is the entry's position in [`Menu::items`], and is what
    /// a later `triggered_index` carries. It is invalidated by any insertion or
    /// removal before it.
    pub fn add_action(&mut self, text: impl Into<String>) -> usize {
        let idx = self.items.len();
        self.items.push(MenuEntry::new(text));
        self.touch_items();
        idx
    }
    /// Appends an action labeled `text` with `shortcut` shown beside it, and
    /// returns its index.
    ///
    /// `shortcut` is display text, not a parsed accelerator — see
    /// [`MenuEntry::with_shortcut`].
    pub fn add_action_with_shortcut(
        &mut self,
        text: impl Into<String>,
        shortcut: impl Into<String>,
    ) -> usize {
        let idx = self.items.len();
        self.items.push(MenuEntry::new(text).with_shortcut(shortcut));
        self.touch_items();
        idx
    }

    /// Appends a row built from a live [`Action`] and returns its index.
    ///
    /// Label, shortcut label, checkability, checked and enabled state, icon, status hint and
    /// command id are read from the action, so the menu is a view of the action rather than a
    /// second declaration of it. The row also records the action's widget id in
    /// [`MenuEntry::action`], which is what lets a host find the live action again; activation
    /// still emits [`Self::action_triggered`] with the command id.
    pub fn add_action_item(&mut self, action: &Action) -> usize {
        let index = self.items.len();
        self.items.push(menu_entry_from_action(action));
        self.touch_items();
        self.base.request_redraw();
        index
    }

    /// Removes the first row linked to `action_id` and returns whether one was removed.
    ///
    /// The id is the registry command id a row was built with — the same string
    /// [`Self::action_triggered`] carries. Rows without an id never match.
    pub fn remove_action(&mut self, action_id: &str) -> bool {
        let Some(index) = self.items.iter().position(|item| item.action_id() == Some(action_id))
        else {
            return false;
        };
        self.remove_item(index).is_some()
    }

    /// Inserts a pre-built entry at `index` and returns the index it actually landed on.
    ///
    /// An index past the end appends, matching the clamping convention of the other index
    /// setters in this control.
    pub fn insert_item(&mut self, index: usize, item: MenuEntry) -> usize {
        let index = index.min(self.items.len());
        self.items.insert(index, item);
        if let Some(hovered) = self.hovered_index {
            if hovered >= index {
                self.hovered_index = Some(hovered + 1);
            }
        }
        if index < self.scroll_offset {
            self.scroll_offset += 1;
        }
        self.submenu_path.clear();
        self.touch_items();
        self.base.request_redraw();
        index
    }

    /// Inserts a separator at `index` and returns the index it landed on.
    pub fn insert_separator(&mut self, index: usize) -> usize {
        self.insert_item(index, MenuEntry::separator())
    }

    /// Removes the entry at `index`, returning it when the index was in range.
    ///
    /// The highlight, the scroll window and the open cascade are adjusted with the list: a
    /// cursor left pointing at the removed row would name a different entry afterwards.
    pub fn remove_item(&mut self, index: usize) -> Option<MenuEntry> {
        if index >= self.items.len() {
            return None;
        }
        let removed = self.items.remove(index);
        self.hovered_index = match self.hovered_index {
            Some(at) if at == index => None,
            Some(at) if at > index => Some(at - 1),
            other => other,
        };
        if self.scroll_offset > index {
            self.scroll_offset -= 1;
        }
        self.submenu_path.clear();
        self.clamp_scroll();
        self.touch_items();
        self.base.request_redraw();
        Some(removed)
    }

    /// Marks the cached row boxes stale. Every mutation of `items` must call this.
    fn touch_items(&mut self) {
        self.items_version = self.items_version.wrapping_add(1);
        self.sync_panel();
    }

    /// Attaches a flat `MenuPanel` mirroring this menu's top-level rows.
    ///
    /// Returns whether a panel is attached afterwards. The panel is a *view* of the menu's model:
    /// it is refilled on every item mutation, so a context menu and an externally placed panel
    /// cannot show different rows (gap 5). A separator and a section caption become a disabled
    /// row, because the panel's model has no separator kind and dropping them would shift every
    /// later row's index relative to the menu's.
    pub fn attach_panel(&mut self, panel: Option<crate::widget::companions::MenuPanel>) -> bool {
        self.panel = panel;
        self.sync_panel();
        self.panel.is_some()
    }

    /// The attached flat panel, if any.
    pub fn panel(&self) -> Option<&crate::widget::companions::MenuPanel> {
        self.panel.as_ref()
    }

    /// The attached flat panel for editing, if any.
    pub fn panel_mut(&mut self) -> Option<&mut crate::widget::companions::MenuPanel> {
        self.panel.as_mut()
    }

    /// Refills the attached panel from the menu's own rows.
    fn sync_panel(&mut self) {
        use crate::widget::companions::MenuPanelItem;

        if self.panel.is_none() {
            return;
        }
        let items: Vec<MenuPanelItem> = self
            .items
            .iter()
            .filter(|item| !item.is_separator())
            .map(|item| {
                let icon = match item.icon() {
                    Some(crate::widget::IconSource::Builtin(name)) => {
                        Some(name.as_str().to_string())
                    }
                    _ => None,
                };
                let mut row = MenuPanelItem::new(item.text());
                row.icon = icon;
                row.enabled = item.is_enabled() && !item.is_section();
                row
            })
            .collect();
        if let Some(panel) = self.panel.as_mut() {
            panel.set_items(items);
        }
    }
    /// Enables or disables the entry at `index`.
    ///
    /// An out-of-range index is ignored silently; read [`Menu::item_enabled`]
    /// afterwards to confirm the change took effect.
    pub fn set_item_enabled(&mut self, index: usize, enabled: bool) {
        let mut changed = false;
        if let Some(item) = self.items.get_mut(index) {
            if item.is_enabled() != enabled {
                item.set_enabled(enabled);
                changed = true;
            }
        }
        if changed {
            // A state change that is not repainted is invisible until something else
            // happens to damage the popup (M-002), and an attached panel is a second view of
            // the same row.
            self.base.request_redraw();
            self.sync_panel();
        }
    }
    /// Returns enabled state for item at index.
    ///
    /// `None` when `index` is out of range.
    pub fn item_enabled(&self, index: usize) -> Option<bool> {
        self.items.get(index).map(|item| item.is_enabled())
    }
    /// Sets the check mark of the entry at `index`.
    ///
    /// Does nothing, silently, when the index is out of range **or** when the
    /// entry is not checkable — see [`MenuEntry::set_checkable`].
    pub fn set_item_checked(&mut self, index: usize, checked: bool) {
        let mut changed = false;
        if let Some(item) = self.items.get_mut(index) {
            if item.is_checkable() && item.is_checked() != checked {
                item.set_checked(checked);
                changed = true;
            }
        }
        if changed {
            // The tick is drawn from the entry, so the popup must repaint (M-002).
            self.base.request_redraw();
        }
    }
    /// Returns checked state for item at index.
    ///
    /// `None` when `index` is out of range. A non-checkable entry reports its
    /// stored flag rather than `None`, so this does not reveal whether the entry
    /// is togglable.
    pub fn item_checked(&self, index: usize) -> Option<bool> {
        self.items.get(index).map(|item| item.is_checked())
    }
    /// Removes every entry. The title and popup position are left alone, so a
    /// visible owner of this menu is re-laid out as empty rather than closed.
    pub fn clear(&mut self) {
        self.items.clear();
        self.scroll_offset = 0;
        self.submenu_path.clear();
        self.hovered_index = None;
        self.touch_items();
    }
    /// The configured cap on visible entries, or `usize::MAX` when the policy is off.
    ///
    /// Reading the policy per call is deliberate: a menu draws once per frame and the
    /// read is an uncontended lock plus a small copy, while caching it in the widget
    /// would make a mid-session policy change invisible until something else invalidated
    /// the menu.
    pub(super) fn visible_capacity() -> usize {
        effects::current()
            .max_visible_items
            .map(|value| value.max(1) as usize)
            .unwrap_or(usize::MAX)
    }

    /// Clamps the scroll offset to the list, so removing entries (or a lower cap) cannot
    /// leave a menu scrolled past its own content.
    pub(super) fn clamp_scroll(&mut self) {
        let capacity = Self::visible_capacity();
        let max_offset = self.items.len().saturating_sub(capacity);
        self.scroll_offset = self.scroll_offset.min(max_offset);
    }

    /// The entries the popup paints, in list order.
    pub(super) fn visible_range(&self) -> core::ops::Range<usize> {
        let capacity = Self::visible_capacity();
        let start = self.scroll_offset.min(self.items.len());
        let end = start.saturating_add(capacity).min(self.items.len());
        start..end
    }

    /// Keeps the highlighted entry inside the visible window after keyboard navigation.
    fn ensure_hovered_visible(&mut self) {
        let Some(index) = self.hovered_index else { return };
        let capacity = Self::visible_capacity();
        if capacity == usize::MAX {
            return;
        }
        if index < self.scroll_offset {
            self.scroll_offset = index;
        } else if index >= self.scroll_offset.saturating_add(capacity) {
            self.scroll_offset = index.saturating_add(1).saturating_sub(capacity);
        }
        self.clamp_scroll();
    }

    /// Whether the panel opened by row `index` of `level` is already showing.
    fn branch_is_open(&self, level: usize, index: usize) -> bool {
        self.submenu_path.get(level).is_some_and(|step| step.row == index)
    }

    /// Arms the hover-open delay for the branch row `index` of `level`.
    ///
    /// A branch already on screen is left alone, so resting on the row that opened the panel
    /// does not restart a timer that has nothing left to do.
    pub(super) fn note_hover_branch(&mut self, level: usize, index: usize) {
        if self.branch_is_open(level, index) {
            self.clear_hover_branch();
            return;
        }
        if self.hover_open_pending == Some((level, index)) {
            return;
        }
        self.hover_open_pending = Some((level, index));
        self.hover_open_elapsed_ms = 0;
    }

    /// Disarms a pending hover-open, so moving off a branch cancels it.
    pub(super) fn clear_hover_branch(&mut self) {
        self.hover_open_pending = None;
        self.hover_open_elapsed_ms = 0;
    }
    /// The height of the title heading the menu draws above its popup body.
    pub(super) fn heading_height() -> f32 {
        20.0
    }

    /// The heading height this menu actually paints: zero when the title is suppressed.
    pub(super) fn drawn_heading_height(&self) -> f32 {
        if self.show_heading {
            Self::heading_height()
        } else {
            0.0
        }
    }

    /// Shows or hides the title heading above the entries.
    ///
    /// [`crate::create_menu`] clears it for a menu owned by a menu bar, whose title the bar
    /// already paints; a context menu keeps it. Changing the flag invalidates the popup because
    /// every row's drawn and clickable position moves by the strip's height.
    pub fn set_show_heading(&mut self, show: bool) {
        if self.show_heading != show {
            self.show_heading = show;
            self.base.request_redraw();
        }
    }

    /// Whether the title heading is painted.
    pub fn shows_heading(&self) -> bool {
        self.show_heading
    }

    /// Makes the popup paint a tear-off strip above its rows.
    ///
    /// The strip is a drag handle a host can use to detach a copy of the menu. It is
    /// off by default, so an ordinary menu's rows do not move.
    pub fn set_tearoff(&mut self, tearoff: bool) {
        if self.tearoff == tearoff {
            return;
        }
        self.tearoff = tearoff;
        self.base.request_redraw();
    }

    /// Whether the popup paints a tear-off strip.
    pub fn tears_off(&self) -> bool {
        self.tearoff
    }

    /// The height of the tear-off strip in popup coordinates.
    pub(super) fn tearoff_band_height(&self) -> u32 {
        if self.tearoff {
            8
        } else {
            0
        }
    }

    /// Enables mnemonic parsing: `&` markers are removed from the drawn label and the
    /// marked letter is underlined.
    ///
    /// Off by default. Alt+letter activation works regardless; this flag only changes
    /// the painted label.
    pub fn set_show_mnemonics(&mut self, show: bool) {
        if self.show_mnemonics == show {
            return;
        }
        self.show_mnemonics = show;
        self.base.request_redraw();
    }

    /// Whether mnemonic markers are stripped and underlined when painted.
    pub fn shows_mnemonics(&self) -> bool {
        self.show_mnemonics
    }

    /// The index of the first enabled entry whose mnemonic is `letter`.
    ///
    /// Matching is case-insensitive, which is what lets one declaration answer both
    /// Alt+F and Alt+Shift+F.
    pub fn mnemonic_index(&self, letter: char) -> Option<usize> {
        let wanted = letter.to_ascii_lowercase();
        self.items.iter().position(|item| {
            item.is_selectable() && item.mnemonic().map(|m| m.to_ascii_lowercase()) == Some(wanted)
        })
    }

    /// Runs the entry whose mnemonic is `letter`, exactly as a click on it would.
    ///
    /// Returns `false` when no enabled entry carries that mnemonic. A branch opens its
    /// nested panel instead of committing, so an accelerator cannot fire a command the
    /// user did not choose.
    pub fn activate_mnemonic(&mut self, letter: char) -> bool {
        let Some(index) = self.mnemonic_index(letter) else {
            return false;
        };
        self.hovered_index = Some(index);
        self.activate_hovered()
    }

    /// The entry of the menu's own list whose cascading panel is open, if any.
    ///
    /// With several levels open this still names the top-level branch; [`Self::submenu_depth`]
    /// reports how far the cascade reaches.
    pub fn open_submenu_index(&self) -> Option<usize> {
        self.submenu_path.first().map(|level| level.row)
    }

    /// Opens the nested panel of `index` in the menu's own list, when that entry owns children.
    ///
    /// Returns `false` for a leaf, an out-of-range index, or a disabled entry, and
    /// leaves any panel that is already open untouched in that case.
    pub fn open_submenu(&mut self, index: usize) -> bool {
        self.open_submenu_at(0, index)
    }

    /// The number of open nested panels, `0` to [`MAX_SUBMENU_DEPTH`].
    pub fn submenu_depth(&self) -> usize {
        self.submenu_path.len()
    }

    /// Closes the deepest nested panel, if one is open. Returns whether anything changed.
    ///
    /// This unwinds **one** level, which is what makes Escape feel like "back": the panel
    /// whose parent is still open closes first, and the menu itself only on the next press.
    pub fn close_submenu(&mut self) -> bool {
        if self.submenu_path.pop().is_none() {
            return false;
        }
        self.base.request_redraw();
        true
    }

    /// The rows shown at `level`: the menu's own list at level 0, a panel's children below it.
    ///
    /// The rows are read from the entry tree rather than copied, so a check mark or an
    /// enabled flag changed while the panel is open is exactly what the panel shows.
    pub(super) fn level_entries(&self, level: usize) -> Option<&[MenuEntry]> {
        if level == 0 {
            return Some(&self.items);
        }
        let step = self.submenu_path.get(level - 1)?;
        self.level_entries(level - 1)?.get(step.row)?.submenu()
    }

    /// The highlighted row of `level`.
    pub(super) fn level_hovered(&self, level: usize) -> Option<usize> {
        if level == 0 {
            self.hovered_index
        } else {
            self.submenu_path.get(level - 1).and_then(|level| level.hovered)
        }
    }

    /// Sets the highlighted row of `level`.
    pub(super) fn set_level_hovered(&mut self, level: usize, index: Option<usize>) {
        if level == 0 {
            self.hovered_index = index;
        } else if let Some(step) = self.submenu_path.get_mut(level - 1) {
            step.hovered = index;
        }
    }

    /// The level that owns the keyboard: the deepest open panel, or the menu's own list.
    pub(super) fn focus_level(&self) -> usize {
        self.submenu_path.len()
    }

    /// The highlighted row inside the deepest open panel.
    pub fn submenu_hovered_index(&self) -> Option<usize> {
        self.submenu_path.last().and_then(|level| level.hovered)
    }

    /// Opens the nested panel of the row at `index` in `level`.
    ///
    /// Opening replaces `level` and every deeper level, so pointing at a sibling branch
    /// discards the branch that was open beside it. A `level` at or beyond
    /// [`MAX_SUBMENU_DEPTH`] is refused, which is the depth contract shared by the menu
    /// family; the activation path still emits `submenu_requested` for such a row.
    pub(super) fn open_submenu_at(&mut self, level: usize, index: usize) -> bool {
        if level >= MAX_SUBMENU_DEPTH {
            return false;
        }
        let Some(entry) = self.level_entries(level).and_then(|entries| entries.get(index)) else {
            return false;
        };
        if !entry.is_selectable() {
            return false;
        }
        let Some(children) = entry.submenu().filter(|children| !children.is_empty()) else {
            return false;
        };
        let hovered = children.iter().position(MenuEntry::is_selectable);
        self.submenu_path.truncate(level);
        self.submenu_path.push(SubmenuLevel { row: index, hovered });
        self.base.request_redraw();
        true
    }

    /// Moves the highlight of `level` to the next selectable row whose label starts
    /// with `letter`.
    ///
    /// This is the menu's half of the type-ahead `DropdownMenu` already has: a bare
    /// letter or digit jumps to a row, and repeating the same letter cycles forward
    /// from the current row so two rows sharing a first letter are both reachable.
    /// The comparison uses [`first_menu_label_char`], so a `&` mnemonic marker is
    /// skipped but never mistaken for part of the label.
    pub(super) fn type_ahead_at(&mut self, level: usize, letter: char) {
        let wanted = letter.to_ascii_lowercase();
        let Some(count) = self.level_entries(level).map(<[MenuEntry]>::len) else {
            return;
        };
        if count == 0 {
            return;
        }
        let start = match self.level_hovered(level) {
            Some(index) if index + 1 < count => index + 1,
            Some(_) => 0,
            None if level == 0 => self.scroll_offset.min(count - 1),
            None => 0,
        };
        let mut landed = None;
        for offset in 0..count {
            let index = (start + offset) % count;
            let Some(entry) = self.level_entries(level).and_then(|entries| entries.get(index))
            else {
                continue;
            };
            if entry.is_selectable()
                && first_menu_label_char(entry.text())
                    .is_some_and(|first| first.to_ascii_lowercase() == wanted)
            {
                landed = Some(index);
                break;
            }
        }
        let Some(index) = landed else {
            return;
        };
        self.set_level_hovered(level, Some(index));
        if level == 0 {
            self.ensure_hovered_visible();
        }
        self.base.request_redraw();
    }

    /// Moves the highlight of `level` by `delta`, skipping unselectable rows.
    pub(super) fn move_level_hovered(&mut self, level: usize, delta: i32) -> bool {
        if level == 0 {
            return self.move_hovered_vertically(delta);
        }
        let Some(count) = self.level_entries(level).map(<[MenuEntry]>::len) else {
            return false;
        };
        if count == 0 || delta == 0 {
            return false;
        }
        let start = match self.level_hovered(level) {
            Some(index) => index as i32,
            None if delta > 0 => -1,
            None => count as i32,
        };
        let mut index = start;
        for _ in 0..count {
            index += delta;
            if index < 0 {
                index = count as i32 - 1;
            } else if index >= count as i32 {
                index = 0;
            }
            let selectable = self
                .level_entries(level)
                .and_then(|entries| entries.get(index as usize))
                .is_some_and(MenuEntry::is_selectable);
            if selectable {
                self.set_level_hovered(level, Some(index as usize));
                self.base.request_redraw();
                return true;
            }
        }
        false
    }
}
