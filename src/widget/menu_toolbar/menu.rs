// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Menu widget.
//!
//! # The popup's rows are derived, not painted ad hoc
//!
//! BLUE22 §B.8 lists `menu` / `menu_item`'s defect as "the check and arrow paddings written by
//! hand". The *columns* of a row are derived (see `Menu::indicator_box` and
//! `Menu::label_box`, which state the standard menu-item relations), and the *run* of rows is
//! one derivation in `Menu::item_bands` that the painter and every hit path share. The run used
//! to be a `let mut y` inside `draw`; a `FlexLayout` column replaced it for a while, but every row
//! has a fixed height the control owns, so the layout only rediscovered constants — on each draw,
//! which is once per frame while the popup reveals.

use crate::compat::{lock, Arc, Mutex, Rc, RefCell, Vec};
use crate::core::{Color, HorizontalAlignment, ObjectId, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::widget::capability::coercion::{
    expect_bool, expect_string, expect_text_direction, expect_u32, text_direction_to_str,
};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::companions::TooltipManager;
use crate::widget::menu_toolbar::action::Action;
use crate::widget::menu_toolbar::popup_reveal::{PopupReveal, RevealDirection};
use crate::widget::menu_toolbar::tool_bar::child_hover_id;
pub use crate::widget::menu_toolbar::MAX_SUBMENU_DEPTH;
use crate::widget::metrics::{dimensions, effective_font, estimate_text_width};
use crate::widget::{BaseWidget, Draw, IconName, IconSource, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

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

/// The ASCII letter or digit a key code names, for mnemonic accelerators.
///
/// Only letters and digits are mnemonics; every other key belongs to the menu's
/// navigation contract.
fn mnemonic_letter(key: u32) -> Option<char> {
    match key {
        65..=90 => char::from_u32(key),
        48..=57 => char::from_u32(key),
        97..=122 => char::from_u32(key),
        _ => None,
    }
}

/// Width of the gap between two item columns, in logical pixels.
const MENU_COLUMN_GAP: u32 = 8;

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

// A hover source adapter is not needed: [`MenuEntry`] stores its hint directly.

/// The facts [`Menu::popup_height`] is a pure function of.
///
/// Opening a menu used to re-derive the same height on every call while the reveal was in
/// flight. The key is the list revision plus everything the height arithmetic reads, so a
/// cached value cannot survive a change that moves a row or changes the visible window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PopupMetricsKey {
    version: u64,
    width: u32,
    columns: u8,
    tearoff: bool,
    /// The configured visible-item cap (part of `visible_range`).
    capacity: usize,
    /// The resolved font size's bit pattern; the heading measurement is a function of it.
    font_bits: u32,
}

/// One row's mnemonic presentation: the stripped label and the measured underline span.
///
/// Stripping `&` and measuring the prefix are both functions of the label and the font, so
/// they are derived once per list/font revision instead of on every painted frame (G-P06).
struct MnemonicRow {
    /// The label with its marker removed.
    text: String,
    /// The mnemonic letter and the `(prefix width, glyph width)` of its underline.
    underline: Option<(char, u32, u32)>,
}

/// The facts [`Menu::item_bands`] is a pure function of.
///
/// A version counter alongside the geometry is what makes the cache cheap to validate: every
/// entry mutation bumps the counter, so a cached run cannot survive a change that moves a row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ItemBandsKey {
    version: u64,
    width: u32,
    scroll_offset: usize,
    tearoff: bool,
    columns: u8,
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
struct SubmenuLevel {
    /// The row in the parent level that opened this panel.
    row: usize,
    /// The highlighted row inside this panel.
    hovered: Option<usize>,
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
    base: BaseWidget,
    title: String,
    items: Vec<MenuEntry>,
    hovered_index: Option<usize>,
    /// Screen position the popup was opened at, for diagnostics and tests.
    invoker_position: Option<Point>,
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
    reveal: PopupReveal,
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
    scroll_offset: usize,
    /// Cached heading line height, keyed by the font size's bit pattern.
    ///
    /// The heading is one shaped measurement per frame; a menu that is open while an
    /// animation runs paid it on every frame of the reveal. The cache is now unconditional:
    /// selection of the font never changes inside one menu, so gating it on the effect
    /// policy only made the default path pay the measurement.
    heading_line_cache: core::cell::Cell<Option<(u32, u32)>>,
    /// The number of columns the popup arranges its entries into.
    ///
    /// `1` (the default) is the historical single-column list. A value above one splits the
    /// popup's inner width into equal columns and fills them row-major, which is what lets a
    /// menu of short entries stop being needlessly tall.
    column_count: u8,
    /// Where [`Self::open_at`] anchors the popup. See [`PopupDirection`].
    popup_direction: PopupDirection,
    /// Bumped by every entry mutation; [`ItemBandsKey`] compares it to decide whether the
    /// cached row boxes are still valid.
    items_version: u64,
    /// The cached row boxes, keyed by [`ItemBandsKey`].
    item_bands_cache: RefCell<Option<(ItemBandsKey, Rc<Vec<Rect>>)>>,
    /// The cached popup height, keyed by [`PopupMetricsKey`].
    popup_metrics_cache: core::cell::Cell<Option<(PopupMetricsKey, f32)>>,
    /// The cached mnemonic rows, keyed by `(items revision, font size bits)`.
    mnemonic_cache: RefCell<Option<(u64, u32, Rc<Vec<MnemonicRow>>)>>,
    /// The branch row the pointer is resting on, awaiting the hover-open delay.
    hover_open_pending: Option<(usize, usize)>,
    /// How long the pointer has rested on [`Self::hover_open_pending`], in milliseconds.
    hover_open_elapsed_ms: u32,
    /// The open nested panels, deepest last. One step per level, up to
    /// [`MAX_SUBMENU_DEPTH`]; level 0 is the menu's own list.
    submenu_path: Vec<SubmenuLevel>,
    /// Whether the popup paints a tear-off strip above its rows.
    tearoff: bool,
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
    show_mnemonics: bool,
    /// Live state cells for rows built from [`crate::action::ActionManager`].
    ///
    /// Each cell is subscribed to the command's own `toggled` / `enabled_changed` signals, so a
    /// change made through the registry is visible without the menu polling it. The row keeps its
    /// `action_id`; this is the state half of that link.
    live_actions: Vec<(String, Arc<Mutex<crate::action::ActionLiveState>>)>,
    /// The host-side hover/delay/show pipeline for row hints.
    tooltips: TooltipManager,
    /// The derived id currently registered as hovered, if any.
    tooltip_source: Option<ObjectId>,
    /// Monotonic clock fed to the tooltip pipeline from [`Widget::tick`].
    tooltip_now_ms: u64,
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
            hovered_index: None,
            invoker_position: None,
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

    /// Opens this menu as a context menu at `position`.
    ///
    /// `viewport` is the area the popup must stay inside — the widget's parent
    /// surface. The popup is shifted left/up when it would overflow, which is what
    /// keeps a menu opened near the right or bottom edge fully reachable instead of
    /// clipped at the edge.
    ///
    /// The popup's vertical anchor follows [`Self::popup_direction`]. With the default
    /// [`PopupDirection::Auto`] the popup opens downward when it fits and is anchored by its
    /// bottom edge when the remaining space below the invocation point is too small, so a
    /// menu opened near the bottom of the screen flips up instead of being clipped.
    pub fn open_at(&mut self, position: Point, viewport: Rect) {
        // A menu created through `create_menu(.., width = 0, height = 0)` — the shape every
        // documented call site uses — has a zero-width geometry. Opening it as-is painted a
        // popup with no pixels at all, so the click that asked for it looked like a no-op.
        // The popup takes the width it was given when that is usable, and the control's own
        // size hint otherwise.
        let rect = self.geometry();
        let popup_w = rect.width.max(self.size_hint().width).max(1) as i32;
        // The drawn popup is its heading strip **plus** the body; both are part of what must
        // fit inside the viewport. Measuring only the body let the heading hang off the top
        // edge when a menu was opened near it.
        let popup_h = self.drawn_heading_height() as i32 + self.popup_height() as i32;

        // Clamp so the whole popup fits; never go negative, because a popup at a
        // negative coordinate is off-screen on every backend.
        let max_x = (viewport.x + viewport.width as i32).saturating_sub(popup_w);
        let max_y = (viewport.y + viewport.height as i32).saturating_sub(popup_h);
        let x = position.x.clamp(viewport.x, max_x.max(viewport.x));
        let below = viewport.y + viewport.height as i32 - position.y;
        let opens_up = match self.popup_direction {
            PopupDirection::Down => false,
            PopupDirection::Up => true,
            // Flipping only when the space above actually fits avoids trading a clipped
            // bottom for a clipped top on a viewport shorter than the popup itself.
            PopupDirection::Auto => {
                below < popup_h && position.y.saturating_sub(popup_h) >= viewport.y
            }
        };
        let anchor_y = if opens_up { position.y.saturating_sub(popup_h) } else { position.y };
        let y = anchor_y.clamp(viewport.y, max_y.max(viewport.y));

        // The geometry is the rectangle `contains_point`, the hit test and `render_frame`
        // read, so it must be the rectangle `draw` paints rather than the caller-supplied
        // 0x0 box.
        self.set_geometry(Rect::new(x, y, popup_w as u32, popup_h.max(1) as u32));
        self.invoker_position = Some(position);
        // A newly opened popup starts at the top of its list; the previous open may have
        // left it scrolled.
        self.scroll_offset = 0;
        self.clamp_scroll();
        // A new open starts at the top level: a cascade left over from the previous open
        // would describe a branch of a menu the user is no longer looking at.
        self.submenu_path.clear();
        // Pre-select the first actionable entry so keyboard navigation has a
        // starting point; a menu opened with no selection cannot be driven by
        // arrow keys without a first press that has no visible effect.
        self.hovered_index =
            self.items.iter().position(|item| !item.is_separator() && item.is_enabled());
        self.show();
        // A menu is a **command surface**, not a decoration: it must be on screen the moment the
        // user asks for it. The reveal still animates on close, but an opening popup that grew
        // over ~30 frames was reported as "菜单加载好慢" — and macOS shows a menu immediately.
        // `jump_to` also guarantees the first painted frame contains every row, so a click that
        // follows the opening press lands on the row the user can see.
        self.reveal.jump_to(true);
    }

    /// The popup body's top edge: the geometry's `y` plus the heading strip.
    ///
    /// `draw` paints the heading first and lays the entry rows out below it. The pointer
    /// handlers used to start their row walk at `geometry().y`, so every row was read as
    /// `heading_height` pixels higher than it was drawn — a click on the lower half of one
    /// row activated the row below it. One derivation, used by the painter and by the hit
    /// paths, is what keeps them from drifting apart again.
    fn body_top(&self) -> i32 {
        self.geometry().y + self.drawn_heading_height() as i32
    }

    /// The pointer position this menu was opened at, if it was opened via
    /// [`Self::open_at`].
    pub fn invoker_position(&self) -> Option<Point> {
        self.invoker_position
    }

    /// Whether `point` lies inside the popup's drawn area or one of its open panels.
    ///
    /// The drawn height follows the item list, not `geometry().height`, because a
    /// menu is as tall as its entries — using the widget's stored height would make
    /// the "outside" test disagree with what the user sees. A nested panel hangs to the
    /// right of the popup, outside that rectangle, so it is part of the accepted area too:
    /// a press on a child row must reach this control rather than the widget behind it.
    pub fn contains_point(&self, point: Point) -> bool {
        let rect = self.geometry();
        // The drawn extent is the heading **plus** the body. Testing only `popup_height`
        // from `rect.y` both accepted clicks on the heading and rejected clicks on the
        // body's lower rows once the popup was taller than the heading.
        let inside_popup = point.x >= rect.x
            && point.x < rect.x + rect.width as i32
            && point.y >= rect.y
            && point.y < self.body_top() + self.popup_height() as i32;
        inside_popup
            || (1..=self.submenu_path.len())
                .filter_map(|level| self.submenu_panel_rect_at(level))
                .any(|panel| {
                    point.x >= panel.x
                        && point.x < panel.x + panel.width as i32
                        && point.y >= panel.y
                        && point.y < panel.y + panel.height as i32
                })
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
    fn move_hovered_horizontally(&mut self, arrow_right: bool) -> bool {
        // "Right" is +1 in left-to-right; the direction's step conversion is exactly the negation
        // rule, so a third direction would extend one place instead of two.
        let step = if arrow_right { 1 } else { -1 };
        let delta = self.direction.begin_step_to_left_step(step);
        self.move_hovered_vertically(delta)
    }

    /// Triggers the entry the highlight is on, if it can still be chosen.
    fn activate_hovered(&mut self) -> bool {
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
    fn activate_level_hovered(&mut self, level: usize) -> bool {
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
    fn sync_hover_tooltip(&mut self) {
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
        let text = entry.tooltip.clone();
        let text = (!text.is_empty()).then_some(text);
        match text {
            Some(text) => {
                let id = child_hover_id(self.base.id(), index);
                self.tooltips.register(id, text);
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
    fn tooltip_owes_frames(&self) -> bool {
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
    }
    /// Enables or disables the entry at `index`.
    ///
    /// An out-of-range index is ignored silently; read [`Menu::item_enabled`]
    /// afterwards to confirm the change took effect.
    pub fn set_item_enabled(&mut self, index: usize, enabled: bool) {
        if let Some(item) = self.items.get_mut(index) {
            item.set_enabled(enabled);
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
        if let Some(item) = self.items.get_mut(index) {
            if item.is_checkable() {
                item.set_checked(checked);
            }
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
    fn item_height() -> f32 {
        22.0
    }

    /// The configured cap on visible entries, or `usize::MAX` when the policy is off.
    ///
    /// Reading the policy per call is deliberate: a menu draws once per frame and the
    /// read is an uncontended lock plus a small copy, while caching it in the widget
    /// would make a mid-session policy change invisible until something else invalidated
    /// the menu.
    fn visible_capacity() -> usize {
        super::effects::current()
            .max_visible_items
            .map(|value| value.max(1) as usize)
            .unwrap_or(usize::MAX)
    }

    /// Clamps the scroll offset to the list, so removing entries (or a lower cap) cannot
    /// leave a menu scrolled past its own content.
    fn clamp_scroll(&mut self) {
        let capacity = Self::visible_capacity();
        let max_offset = self.items.len().saturating_sub(capacity);
        self.scroll_offset = self.scroll_offset.min(max_offset);
    }

    /// The entries the popup paints, in list order.
    fn visible_range(&self) -> core::ops::Range<usize> {
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

    /// The box of every entry, separators included, in popup coordinates.
    ///
    /// The boxes are in **popup coordinates** — the origin is the popup's top-left, not the
    /// control's — because that is the frame the entries are painted in; the caller translates to
    /// `rect` when it draws.
    ///
    /// The popup's own `MENU_POPUP_PADDING` is the column's padding rather than a `+ 2.0` the paint
    /// loop added before the first row, so the inset is part of the run instead of a constant that
    /// only the first row knows about.
    /// The row boxes, served from a cache keyed by the list version and geometry.
    ///
    /// The run is a pure function of the entries' separator flags, the popup's width, the scroll
    /// offset, the tear-off strip and the column count. Rebuilding it on every paint meant a
    /// `Vec` allocation per frame during the open reveal even though none of those facts had
    /// changed; the version counter is bumped by every entry mutation, so a stale run cannot be
    /// served after an edit.
    fn item_bands(&self) -> Rc<Vec<Rect>> {
        let key = ItemBandsKey {
            version: self.items_version,
            width: self.geometry().width,
            scroll_offset: self.scroll_offset,
            tearoff: self.tearoff,
            columns: self.column_count,
        };
        if let Some((cached_key, bands)) = self.item_bands_cache.borrow().as_ref() {
            if *cached_key == key {
                return Rc::clone(bands);
            }
        }
        let bands = Rc::new(self.compute_item_bands());
        *self.item_bands_cache.borrow_mut() = Some((key, Rc::clone(&bands)));
        bands
    }

    /// Rebuilds the row boxes; [`Self::item_bands`] is the cached entry point.
    fn compute_item_bands(&self) -> Vec<Rect> {
        if self.items.is_empty() {
            return Vec::new();
        }
        // Direct arithmetic, not a `FlexLayout` column: every row has a fixed height the control
        // itself owns (`item_height` / `separator_height`), so a layout engine was asked to
        // rediscover a constant on **every** draw — and during the open reveal that is once per
        // frame. Measured as menu-open latency on the GTK demo. The relations the layout used to
        // state are the same three lines below: the run starts at the popup's padding, each row
        // follows the previous one, and the run is as tall as `popup_height` reports.
        let padding = dimensions::MENU_POPUP_PADDING;
        let available = self.geometry().width.saturating_sub(padding * 2);
        let top = padding as i32 + self.tearoff_band_height() as i32;

        if self.column_count > 1 {
            // Row-major grid: entry `i` owns cell `(i / columns, i % columns)`. Every cell is
            // `item_height` tall, including a separator's, so the columns stay aligned; the
            // single-column branch keeps the historical separator height.
            let columns = self.column_count as u32;
            let gaps = MENU_COLUMN_GAP.saturating_mul(columns - 1);
            let column_width = available.saturating_sub(gaps) / columns;
            let column_width = column_width.max(1);
            let first_row = (self.scroll_offset / columns as usize) as i32;
            let mut placed = Vec::with_capacity(self.items.len());
            for index in 0..self.items.len() {
                let row = (index / columns as usize) as i32 - first_row;
                let column = (index % columns as usize) as u32;
                let x = padding as i32 + (column * (column_width + MENU_COLUMN_GAP)) as i32;
                let y = top + row * Self::item_height() as i32;
                placed.push(Rect::new(x, y, column_width, Self::item_height() as u32));
            }
            return placed;
        }

        let mut placed = Vec::with_capacity(self.items.len());
        // Rows above the window are laid out too (with negative `y`) rather than omitted:
        // the painter, the hit test and the keyboard all address an entry by its **list**
        // index, so a band that disappeared would shift every index below it.
        let skipped: i32 = self
            .items
            .iter()
            .take(self.scroll_offset)
            .map(|item| {
                if item.is_separator() {
                    Self::separator_height() as i32
                } else {
                    Self::item_height() as i32
                }
            })
            .sum();
        let mut y = top - skipped;
        for item in self.items.iter() {
            let height = if item.is_separator() {
                Self::separator_height() as u32
            } else {
                Self::item_height() as u32
            };
            placed.push(Rect::new(padding as i32, y, available, height));
            y += height as i32;
        }
        placed
    }

    /// The mnemonic presentation of every row, derived once per list/font revision.
    ///
    /// Stripping the `&` marker and measuring the prefix used to happen for every visible row on
    /// every frame; both are pure functions of the label and the resolved font, so the result is
    /// cached and invalidated by the same revision that invalidates the row boxes (G-P06).
    fn mnemonic_rows(&self, font: &crate::core::Font) -> Rc<Vec<MnemonicRow>> {
        let font_bits = font.size().to_bits();
        if let Some((version, cached_bits, rows)) = self.mnemonic_cache.borrow().as_ref() {
            if *version == self.items_version && *cached_bits == font_bits {
                return Rc::clone(rows);
            }
        }
        let rows: Vec<MnemonicRow> = self
            .items
            .iter()
            .map(|item| {
                let text = strip_menu_mnemonic(item.text());
                let underline = item.mnemonic().and_then(|mnemonic| {
                    text.char_indices().find(|(_, ch)| ch.eq_ignore_ascii_case(&mnemonic)).map(
                        |(byte, ch)| {
                            let prefix = estimate_text_width(&text[..byte], font, 1.0);
                            let through =
                                estimate_text_width(&text[..byte + ch.len_utf8()], font, 1.0);
                            (mnemonic, prefix, through.saturating_sub(prefix))
                        },
                    )
                });
                MnemonicRow { text, underline }
            })
            .collect();
        let rows = Rc::new(rows);
        *self.mnemonic_cache.borrow_mut() = Some((self.items_version, font_bits, Rc::clone(&rows)));
        rows
    }

    /// Whether the panel opened by row `index` of `level` is already showing.
    fn branch_is_open(&self, level: usize, index: usize) -> bool {
        self.submenu_path.get(level).is_some_and(|step| step.row == index)
    }

    /// Arms the hover-open delay for the branch row `index` of `level`.
    ///
    /// A branch already on screen is left alone, so resting on the row that opened the panel
    /// does not restart a timer that has nothing left to do.
    fn note_hover_branch(&mut self, level: usize, index: usize) {
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
    fn clear_hover_branch(&mut self) {
        self.hover_open_pending = None;
        self.hover_open_elapsed_ms = 0;
    }
    /// The height of the title heading the menu draws above its popup body.
    fn heading_height() -> f32 {
        20.0
    }

    /// The heading height this menu actually paints: zero when the title is suppressed.
    fn drawn_heading_height(&self) -> f32 {
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
    fn tearoff_band_height(&self) -> u32 {
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
    fn level_entries(&self, level: usize) -> Option<&[MenuEntry]> {
        if level == 0 {
            return Some(&self.items);
        }
        let step = self.submenu_path.get(level - 1)?;
        self.level_entries(level - 1)?.get(step.row)?.submenu()
    }

    /// The highlighted row of `level`.
    fn level_hovered(&self, level: usize) -> Option<usize> {
        if level == 0 {
            self.hovered_index
        } else {
            self.submenu_path.get(level - 1).and_then(|level| level.hovered)
        }
    }

    /// Sets the highlighted row of `level`.
    fn set_level_hovered(&mut self, level: usize, index: Option<usize>) {
        if level == 0 {
            self.hovered_index = index;
        } else if let Some(step) = self.submenu_path.get_mut(level - 1) {
            step.hovered = index;
        }
    }

    /// The level that owns the keyboard: the deepest open panel, or the menu's own list.
    fn focus_level(&self) -> usize {
        self.submenu_path.len()
    }

    /// The highlighted row inside the deepest open panel.
    pub fn submenu_hovered_index(&self) -> Option<usize> {
        self.submenu_path.last().and_then(|level| level.hovered)
    }

    /// The deepest nested panel's rectangle, in the menu's own coordinate space.
    ///
    /// It hangs from the parent row's top edge and the parent level's trailing edge. The
    /// panel width is fixed at [`Self::submenu_panel_width`] so the panel cannot shrink
    /// around a short child label.
    pub fn submenu_panel_rect(&self) -> Option<Rect> {
        self.submenu_panel_rect_at(self.submenu_depth())
    }

    /// The width the nested panels paint at.
    pub fn submenu_panel_width() -> u32 {
        180
    }

    /// The height a nested panel needs for `children`.
    fn submenu_panel_height(children: &[MenuEntry]) -> u32 {
        children.iter().map(Self::entry_height).sum::<u32>() + 4
    }

    /// The height one panel row occupies.
    fn entry_height(entry: &MenuEntry) -> u32 {
        if entry.is_separator() || entry.is_section() {
            Self::separator_height() as u32
        } else {
            Self::item_height() as u32
        }
    }

    /// The panel rectangle of `level`, which is `1`-based, in the menu's coordinate space.
    fn submenu_panel_rect_at(&self, level: usize) -> Option<Rect> {
        if level == 0 {
            return None;
        }
        let step = self.submenu_path.get(level - 1)?;
        let anchor = self.row_rect_at(level - 1, step.row)?;
        let width = Self::submenu_panel_width();
        // Every panel hangs 6 px over its parent's trailing edge, the same overlap the first
        // level has always used. Keeping the relation for each level is what makes a
        // four-deep cascade one shape rather than four hardcoded offsets.
        let x = self.geometry().x + self.geometry().width as i32 - 6
            + (level as i32 - 1) * (width as i32 - 6);
        let height = Self::submenu_panel_height(self.level_entries(level)?);
        Some(Rect::new(x, anchor.y, width, height))
    }

    /// Row `row` of `level` in the menu's coordinate space.
    fn row_rect_at(&self, level: usize, row: usize) -> Option<Rect> {
        if level == 0 {
            let band = *self.item_bands().get(row)?;
            return Some(Rect::new(
                self.geometry().x + band.x,
                self.body_top() + band.y,
                band.width,
                band.height,
            ));
        }
        let panel = self.submenu_panel_rect_at(level)?;
        let entries = self.level_entries(level)?;
        let mut y = panel.y + 2;
        for (index, entry) in entries.iter().enumerate() {
            let height = Self::entry_height(entry);
            if index == row {
                return Some(Rect::new(panel.x + 2, y, panel.width.saturating_sub(4), height));
            }
            y += height as i32;
        }
        None
    }

    /// The child row at `pos` across every open panel, deepest first.
    ///
    /// Returns `(level, index)`, where `level` is `1` for the panel attached to the menu's own
    /// list. Panels are laid out side by side, so only one can contain the point; asking
    /// deepest-first keeps the answer honest if that ever changes.
    fn hit_test_submenu(&self, pos: Point) -> Option<(usize, usize)> {
        for level in (1..=self.submenu_path.len()).rev() {
            let Some(panel) = self.submenu_panel_rect_at(level) else {
                continue;
            };
            if pos.x < panel.x
                || pos.x >= panel.x + panel.width as i32
                || pos.y < panel.y
                || pos.y >= panel.y + panel.height as i32
            {
                continue;
            }
            let Some(entries) = self.level_entries(level) else {
                continue;
            };
            let mut y = panel.y + 2;
            for (index, entry) in entries.iter().enumerate() {
                let height = Self::entry_height(entry) as i32;
                if pos.y >= y && pos.y < y + height {
                    return Some((level, index));
                }
                y += height;
            }
        }
        None
    }

    /// The child row at `pos`, when an open panel contains it.
    pub fn submenu_index_at(&self, pos: Point) -> Option<usize> {
        self.hit_test_submenu(pos).map(|(_, index)| index)
    }

    /// Opens the nested panel of the row at `index` in `level`.
    ///
    /// Opening replaces `level` and every deeper level, so pointing at a sibling branch
    /// discards the branch that was open beside it. A `level` at or beyond
    /// [`MAX_SUBMENU_DEPTH`] is refused, which is the depth contract shared by the menu
    /// family; the activation path still emits `submenu_requested` for such a row.
    fn open_submenu_at(&mut self, level: usize, index: usize) -> bool {
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

    /// Moves the highlight of `level` by `delta`, skipping unselectable rows.
    fn move_level_hovered(&mut self, level: usize, delta: i32) -> bool {
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

    /// Paints every open nested panel.
    ///
    /// One panel per open level, hung from its parent row's right edge, deepest last. The rows
    /// use the same indicator/label/trailing derivations as the parent, so a child row aligns
    /// with a parent row even though the panel is narrower.
    #[allow(clippy::too_many_arguments)]
    fn draw_submenu_panel(
        &self,
        context: &mut RenderContext,
        font: &crate::core::Font,
        ink: Color,
        secondary: Color,
        primary: Color,
        border: Color,
        face: Color,
    ) {
        // Every open level paints its own panel, left to right. A panel is laid out from the
        // rows it actually holds, so a section or a separator keeps its own height and the
        // next level anchors to the row the user sees.
        for level in 1..=self.submenu_path.len() {
            let Some(panel) = self.submenu_panel_rect_at(level) else {
                continue;
            };
            let Some(children) = self.level_entries(level) else {
                continue;
            };
            context.fill_rect(panel, face.blend(&ink, 0.16));
            context.draw_rect(panel, border);
            // Content stays inside its own panel: a long child label or a wide icon must clip at
            // the panel edge instead of painting over the level beside it (M-02).
            context.push_clip(panel.x, panel.y, panel.width, panel.height);
            let mut y = panel.y + 2;
            for (index, child) in children.iter().enumerate() {
                let height = if child.is_separator() || child.is_section() {
                    Self::separator_height() as u32
                } else {
                    Self::item_height() as u32
                };
                let row = Rect::new(panel.x + 2, y, panel.width.saturating_sub(4), height);
                if child.is_separator() {
                    context.draw_line(
                        Point::new(row.x + 2, y + height as i32 / 2),
                        Point::new(row.x + row.width as i32 - 2, y + height as i32 / 2),
                        border,
                    );
                    y += height as i32;
                    continue;
                }
                if child.is_section() {
                    let band = Rect::new(row.x + 8, row.y, row.width.saturating_sub(8), row.height);
                    context.draw_text_fitted_cow(
                        context.text_line(band, font),
                        child.text(),
                        font,
                        secondary,
                        HorizontalAlignment::Left,
                    );
                    y += height as i32;
                    continue;
                }
                let hovered = self.level_hovered(level) == Some(index);
                if hovered {
                    context.fill_rect(row, primary);
                }
                let fg = if !child.is_enabled() {
                    self.base.disabled_ink_on(ink, face)
                } else if hovered {
                    primary.contrast_color()
                } else {
                    ink
                };
                let indicator = self.indicator_box(row);
                let label = self.label_box(row, indicator);
                if let Some(source) = child.icon() {
                    let line = context.text_line(indicator, font);
                    let side = line.height.max(1).min(indicator.width.max(1));
                    let icon_rect = Rect::new(
                        indicator.x + (indicator.width as i32 - side as i32) / 2,
                        line.y,
                        side,
                        side,
                    );
                    if let Err(error) =
                        crate::widget::paint_icon(context, source.into(), icon_rect, fg)
                    {
                        log::warn!("submenu entry icon could not be painted: {error:?}");
                    }
                } else if child.is_checkable() && child.is_checked() {
                    let line = context.text_line(indicator, font);
                    let side = line.height.max(1);
                    let tick = Rect::new(
                        indicator.x + (indicator.width as i32 - side as i32) / 2,
                        line.y,
                        side,
                        side,
                    );
                    crate::widget::draw_icon_at(context, tick, fg, IconName::Check);
                }
                context.draw_text_fitted_cow(
                    context.text_line(label, font),
                    child.text(),
                    font,
                    fg,
                    HorizontalAlignment::Left,
                );
                if child.has_submenu() && level < MAX_SUBMENU_DEPTH {
                    let arrow_box = self.submenu_arrow_box(row, label);
                    let side = arrow_box.width.min(arrow_box.height).min(16);
                    crate::widget::draw_icon_centered(
                        context,
                        arrow_box,
                        side,
                        fg,
                        IconName::ChevronRight,
                    );
                }
                y += height as i32;
            }
            context.pop_clip();
        }
    }
    fn separator_height() -> f32 {
        6.0
    }

    /// The box the check/radio indicator occupies at a row's leading edge.
    ///
    /// # Why the indicator is a box and not an `8.0` literal
    ///
    /// The check column, the label column and the two trailing columns used to be four
    /// independent literals in `draw` (`x + 8`, `x + 28`, `right - 8`, `right - 4`), which is
    /// the exact shape BLUE22 §B.8 lists for this control: with literals, an indicator or an
    /// arrow that changes size does not *push* its neighbour, it overlaps it. The standard relation is stated
    /// directly —
    /// `leftPadding: padding + (checkable ? indicator.width + spacing : 0)` — so the label's
    /// box is whatever the indicator leaves, and the shortcut's box is whatever the submenu
    /// arrow leaves.
    fn indicator_box(&self, row: Rect) -> Rect {
        let size = dimensions::CHECKBOX_BOX.min(row.width).min(row.height);
        Rect::new(
            row.x + dimensions::MENU_ROW_PADDING_H as i32,
            row.y + (row.height as i32 - size as i32) / 2,
            size,
            size,
        )
    }

    /// The box a row's label may occupy: the row's interior after the indicator column.
    ///
    /// Derived from [`Self::indicator_box`], so a wider indicator narrows the label rather
    /// than letting the two overlap. The column is reserved for **every** row, checkable or
    /// not, because a menu whose labels shift sideways depending on whether a *sibling* has a
    /// tick is the classic menu misalignment.
    fn label_box(&self, row: Rect, indicator: Rect) -> Rect {
        let left = indicator.x + indicator.width as i32 + dimensions::INDICATOR_TEXT_SPACING as i32;
        let right = self.trailing_column_x(row);
        Rect::new(left, row.y, right.saturating_sub(left).max(0) as u32, row.height)
    }

    /// The x where the trailing columns (shortcut, submenu arrow) begin.
    fn trailing_column_x(&self, row: Rect) -> i32 {
        row.x + row.width as i32 - self.trailing_width() as i32
    }

    /// Width of the column the shortcut / submenu arrow occupies.
    ///
    /// The constant (`MENU_ROW_TRAILING_WIDTH`) is the *minimum*, sized for a submenu arrow.
    /// A shortcut is text and can be wider than that — `Ctrl+Shift+Z` at body size is three
    /// times the arrow's column — and the old fixed reserve handed `draw_text_fitted` a box
    /// narrower than the string, so every shortcut in the demo painted as "…". Measuring the
    /// widest shortcut in **this** menu keeps the column honest for the entries it actually
    /// holds, and keeps the label box from stealing the text's room.
    fn trailing_width(&self) -> u32 {
        let font = effective_font(self.style());
        let widest = self
            .items
            .iter()
            .filter(|item| self.row_shows_shortcut(item))
            .map(|item| {
                estimate_text_width(item.shortcut(), font, 1.0) + 2 * crate::render::TEXT_FIT_MARGIN
            })
            .max()
            .unwrap_or(0);
        widest.max(dimensions::MENU_ROW_TRAILING_WIDTH)
    }

    /// The box the submenu arrow may occupy: what is left of the row after the label.
    ///
    /// Derived from the **label's own right edge** rather than from the row's trailing column, so a
    /// longer label shortens the arrow's room instead of the two being placed from opposite ends of
    /// the row and colliding in the middle. Extracted from the draw loop so the layout this test
    /// asserts and the layout that draws are one derivation (`principle #23`), and so a row wide
    /// enough for a shortcut *and* an arrow can be checked as a set of disjoint columns.
    fn submenu_arrow_box(&self, row: Rect, label: Rect) -> Rect {
        let left = label.x + label.width as i32;
        Rect::new(left, row.y, (row.x + row.width as i32 - left).max(0) as u32, row.height)
    }

    /// Whether `item`'s row shows its shortcut in the trailing column.
    ///
    /// # Why this predicate exists
    ///
    /// The trailing column is **shared** by the shortcut and the submenu arrow: one 28 px reserve
    /// rather than two, because a row was documented to show at most one of them at a time
    /// (`dimensions::MENU_ROW_TRAILING_WIDTH`). Nothing enforced that, and the draw path placed the
    /// shortcut right-aligned and the arrow centred in the *same* box — so an entry given both drew
    /// them on top of each other. A row is not an unusual thing to configure that way: a submenu entry
    /// with a shortcut is legal to construct.
    ///
    /// The arrow wins, and the reason is asymmetric: the arrow is the affordance that says "this
    /// reveals a subtree", and hiding it makes the subtree unreachable by pointer, whereas the
    /// constant's own documentation records that a shortcut on such a row is not useful. Stating the
    /// precedence here means the layout, the draw and the test read one rule instead of three.
    fn row_shows_shortcut(&self, item: &MenuEntry) -> bool {
        !item.shortcut().is_empty() && !item.has_submenu()
    }
    fn _item_rect(&self, index: usize, base_y: f32) -> Rect {
        let rect = self.geometry();
        let mut y = base_y;
        for (i, item) in self.items.iter().enumerate() {
            let h =
                if item.is_separator() { Self::separator_height() } else { Self::item_height() };
            if i == index {
                return Rect { x: rect.x, y: y as i32, width: rect.width, height: h as u32 };
            }
            y += h;
        }
        Rect { x: 0, y: 0, width: 0, height: 0 }
    }
    fn popup_height(&self) -> f32 {
        // The height is a pure function of the visible window and the row kinds, so it is
        // cached against the list revision and the geometry that selects the window. Opening
        // a menu while its reveal animates read this on every frame (G-P01).
        let key = PopupMetricsKey {
            version: self.items_version,
            width: self.geometry().width,
            columns: self.column_count,
            tearoff: self.tearoff,
            capacity: Self::visible_capacity(),
            font_bits: effective_font(self.style()).size().to_bits(),
        };
        if let Some((cached_key, height)) = self.popup_metrics_cache.get() {
            if cached_key == key {
                return height;
            }
        }
        let height = self.compute_popup_height();
        self.popup_metrics_cache.set(Some((key, height)));
        height
    }

    /// Rebuilds the popup height; [`Self::popup_height`] is the cached entry point.
    fn compute_popup_height(&self) -> f32 {
        let range = self.visible_range();
        let tearoff = self.tearoff_band_height() as f32;
        if self.column_count > 1 {
            let visible = range.len();
            let rows = visible.div_ceil(self.column_count as usize);
            return rows as f32 * Self::item_height() + 4.0 + tearoff;
        }
        self.items[range]
            .iter()
            .map(
                |item| {
                    if item.is_separator() {
                        Self::separator_height()
                    } else {
                        Self::item_height()
                    }
                },
            )
            .sum::<f32>()
            + 4.0
            + tearoff
    }

    /// Paints the popup's elevation shadow.
    ///
    /// `blur` in [`crate::style::MenuEffects`] is the shadow's blur radius: the renderer has
    /// no way to sample the backdrop, so a backdrop blur is not available (the same boundary
    /// `Material::Translucent` documents). Without hardware acceleration the shadow is a flat
    /// offset rect; the blurred path is the expensive one that flag gates.
    /// The entry drawn at `pos`, if any.
    ///
    /// Uses the same bands the painter uses, which is what keeps a click on the row the user
    /// can see from landing on the row above it after a scroll.
    fn item_index_at(&self, pos: Point) -> Option<usize> {
        if !self.contains_point(pos) {
            return None;
        }
        let bands = self.item_bands();
        let body_top = self.body_top();
        let left = self.geometry().x;
        for index in self.visible_range() {
            let Some(row) = bands.get(index) else { continue };
            let top = body_top + row.y;
            // A single-column menu historically addressed a row by its y band alone, which is
            // what makes the popup's horizontal padding part of the row. A grid has real
            // columns, so it must test x as well or every column would answer for the first.
            let hit = if self.column_count > 1 {
                pos.x >= left + row.x
                    && pos.x < left + row.x + row.width as i32
                    && pos.y >= top
                    && pos.y < top + row.height as i32
            } else {
                pos.y >= top && pos.y < top + row.height as i32
            };
            if hit {
                return Some(index);
            }
        }
        None
    }
}
impl Widget for Menu {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        // The cached heading line height is a function of the resolved font; a theme switch
        // can change the family/weight at the same size, so the cache is dropped with it.
        self.heading_line_cache.set(None);
        self.popup_metrics_cache.set(None);
        *self.mnemonic_cache.borrow_mut() = None;
        crate::style::reapply_active_theme_state(self);
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn has_open_popup(&self) -> bool {
        self.is_visible()
    }

    /// The popup and every open nested panel accept the pointer.
    ///
    /// The panels hang to the right of the popup, outside its geometry; the default
    /// implementation tests only the widget rectangle, so the runtime's hit test rejected
    /// every press on a child row before the control could see it.
    fn contains_point(&self, point: Point) -> bool {
        Self::contains_point(self, point)
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(200, 200)
    }

    fn show(&mut self) {
        // Every open path starts at the top of the list. A menu bar reopens with `show()`
        // rather than `open_at`, so this is the one place both paths share; resetting on
        // close instead would jump the rows while the closing animation is still painting.
        self.scroll_offset = 0;
        self.clamp_scroll();
        self.about_to_show.emit();
        self.reveal.set_open(true);
        self.base.show();
    }
    fn hide(&mut self) {
        // The reveal is aimed at closed **before** the flag flips, so the closing transition is
        // still animated: `draw` paints the body while the reveal has anything left to show, and
        // only a fully collapsed reveal stops it.
        self.reveal.set_open(false);
        self.tooltip_source = None;
        self.tooltips.note_leave();
        self.base.hide();
        // Clear the invoker position with the popup: it describes an open menu, and
        // leaving it set would make a reopened menu report a stale origin.
        self.invoker_position = None;
        self.hovered_index = None;
        self.clear_hover_branch();
        self.submenu_path.clear();
        self.about_to_hide.emit();
    }

    /// Advances the popup's open/close reveal; `true` while it still owes frames.
    ///
    /// Forwarded to the trait so the animation bus drives it — `tick_animations` sweeps every
    /// mounted control, so a menu that kept its tick inherent would be one the host cannot
    /// advance.
    fn tick(&mut self, delta_ms: u32) -> bool {
        self.tooltip_now_ms = self.tooltip_now_ms.saturating_add(u64::from(delta_ms));
        let tooltip_changed = self.tooltips.tick(u64::from(delta_ms), self.tooltip_now_ms);
        // A branch the pointer is resting on opens once the shared delay elapses. The pending
        // pair is what makes this a delay rather than an immediate open on every pass-over.
        let mut hover_changed = false;
        if let Some((level, index)) = self.hover_open_pending {
            self.hover_open_elapsed_ms = self.hover_open_elapsed_ms.saturating_add(delta_ms);
            if self.hover_open_elapsed_ms >= super::effects::SUBMENU_HOVER_DELAY_MS {
                self.hover_open_pending = None;
                self.hover_open_elapsed_ms = 0;
                hover_changed = self.open_submenu_at(level, index);
            } else {
                hover_changed = true;
            }
        }
        self.reveal.tick(delta_ms) || tooltip_changed || self.tooltip_owes_frames() || hover_changed
    }

    fn is_animating(&self) -> bool {
        self.reveal.is_animating()
            || self.tooltip_owes_frames()
            || self.hover_open_pending.is_some()
    }

    impl_draw_bridge!();
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
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "triggered" => Some(EventSignalRef::mapped("triggered", &self.triggered, |v| {
                CapabilityValue::String(v.clone())
            })),
            "triggered_index" => {
                Some(EventSignalRef::mapped("triggered_index", &self.triggered_index, |v| {
                    CapabilityValue::UInt(*v as u64)
                }))
            }
            "about_to_show" => Some(EventSignalRef::unit("about_to_show", &self.about_to_show)),
            "submenu_requested" => {
                Some(EventSignalRef::mapped("submenu_requested", &self.submenu_requested, |v| {
                    CapabilityValue::String(v.clone())
                }))
            }
            "about_to_hide" => Some(EventSignalRef::unit("about_to_hide", &self.about_to_hide)),
            "tearoff_requested" => {
                Some(EventSignalRef::unit("tearoff_requested", &self.tearoff_requested))
            }
            "action_triggered" => {
                Some(EventSignalRef::mapped("action_triggered", &self.action_triggered, |v| {
                    CapabilityValue::String(v.clone())
                }))
            }
            _ => None,
        }
    }
}

/// `Menu`'s property contract.
///
/// Only `title` is writable; `item_count` and `hovered_index` describe the menu's
/// current contents and pointer state, so they are read-only. `hovered_index` is
/// optional and published as `Null` when nothing is hovered.
impl WidgetProperties for Menu {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "title" => Ok(CapabilityValue::String(self.title().to_string())),
            "item_count" => Ok(CapabilityValue::UInt(self.items().len() as u64)),
            "hovered_index" => match self.hovered_index() {
                Some(idx) => Ok(CapabilityValue::UInt(idx as u64)),
                None => Ok(CapabilityValue::Null),
            },
            "direction" => {
                Ok(CapabilityValue::String(text_direction_to_str(self.direction()).to_string()))
            }
            "show_heading" => Ok(CapabilityValue::Bool(self.shows_heading())),
            "column_count" => Ok(CapabilityValue::UInt(self.columns() as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "title" => {
                self.set_title(expect_string(value)?);
                Ok(())
            }
            "direction" => {
                self.set_direction(expect_text_direction(value)?);
                Ok(())
            }
            "show_heading" => {
                self.set_show_heading(expect_bool(value)?);
                Ok(())
            }
            "column_count" => {
                let columns = expect_u32(value)?.clamp(1, u8::MAX as u32) as u8;
                self.set_column_count(columns);
                Ok(())
            }
            "hovered_index" => Err(CapabilityAccessError::ReadOnlyProperty),
            "item_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "title",
            "item_count",
            "hovered_index",
            "direction",
            "show_heading",
            "column_count",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `menu` publishes.
    ///
    /// `clear` is the one genuine zero-argument action here: it drops every entry,
    /// which is exactly what [`Menu::clear`] does. `add_action` and `add_separator`
    /// append an entry and therefore need the label (or the entry) the caller wants
    /// appended, so a payload-less invocation is refused as
    /// [`CapabilityAccessError::OutOfRange`] — the names are right and the argument
    /// is what is missing, which is not `UnknownCommand`.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "clear" => {
                self.clear();
                Ok(())
            }
            "add_action" | "add_separator" => Err(CapabilityAccessError::OutOfRange),
            // Any other `set_foo` name carries its value through the property route,
            // so the shared default reports that a payload is needed rather than
            // claiming the control has never heard of it.
            _ if name.starts_with("set_") => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for Menu {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MouseMove { pos } => {
                self.tooltips.note_pointer(*pos);
                if let Some((level, index)) = self.hit_test_submenu(*pos) {
                    if self
                        .level_entries(level)
                        .and_then(|entries| entries.get(index))
                        .is_some_and(MenuEntry::is_selectable)
                    {
                        self.set_level_hovered(level, Some(index));
                        let is_branch = self
                            .level_entries(level)
                            .and_then(|entries| entries.get(index))
                            .is_some_and(MenuEntry::has_submenu);
                        if is_branch {
                            self.note_hover_branch(level, index);
                        } else {
                            self.clear_hover_branch();
                        }
                        self.base.request_redraw();
                    }
                } else if let Some(index) = self.item_index_at(*pos) {
                    if self.items[index].is_selectable() {
                        self.hovered_index = Some(index);
                        // Hovering a branch opens its panel after a short rest (M-01); a
                        // leaf cancels any pending branch the pointer just left.
                        if self.items[index].has_submenu() {
                            self.note_hover_branch(0, index);
                        } else {
                            self.clear_hover_branch();
                        }
                    }
                } else {
                    self.clear_hover_branch();
                }
                self.sync_hover_tooltip();
            }
            Event::MouseLeave { .. } => {
                self.hovered_index = None;
                self.tooltip_source = None;
                self.clear_hover_branch();
                self.tooltips.note_leave();
            }
            Event::MousePress { pos, button: 1, .. } => {
                // The nested panels are outside the parent's own rectangle, so they are
                // hit tested first: otherwise a click on a child row would read as
                // "outside the popup" and dismiss the whole menu.
                if let Some((level, index)) = self.hit_test_submenu(*pos) {
                    if self
                        .level_entries(level)
                        .and_then(|entries| entries.get(index))
                        .is_some_and(MenuEntry::is_selectable)
                    {
                        self.set_level_hovered(level, Some(index));
                        self.activate_level_hovered(level);
                    }
                    return;
                }
                let tearoff_band = self.tearoff_band_height();
                if tearoff_band > 0 {
                    let top = self.body_top();
                    if pos.y >= top && pos.y < top + tearoff_band as i32 {
                        self.tearoff_requested.emit();
                        return;
                    }
                }
                if let Some(index) = self.item_index_at(*pos) {
                    let item = &self.items[index];
                    if item.is_selectable() {
                        self.hovered_index = Some(index);
                        // A submenu parent is a branch, not a leaf: report it as one and leave the
                        // popup open rather than firing a command the user did not choose (G-3).
                        // A click is the *first* way a user meets this defect -- they click `Cut >`
                        // and the `Cut` action runs -- so this arm matters more than the key one.
                        self.activate_hovered();
                    }
                }
                // A press inside the popup that missed every entry still belongs to
                // the menu (a click on its padding), so it must not dismiss it.
                if !self.contains_point(*pos) {
                    self.hide();
                }
            }
            // A context menu is dismissed by a press anywhere outside it, whichever
            // button was used. Restricting this to the secondary button left the menu
            // open after the most common dismissal gesture, a left-click elsewhere.
            Event::MousePress { pos, .. } if !self.contains_point(*pos) => {
                self.hide();
            }
            #[cfg(feature = "touch")]
            Event::Tap { pos } => {
                if let Some((level, index)) = self.hit_test_submenu(*pos) {
                    if self
                        .level_entries(level)
                        .and_then(|entries| entries.get(index))
                        .is_some_and(MenuEntry::is_selectable)
                    {
                        self.set_level_hovered(level, Some(index));
                        self.activate_level_hovered(level);
                    }
                } else if let Some(index) = self.item_index_at(*pos) {
                    if self.items[index].is_selectable() {
                        self.hovered_index = Some(index);
                        self.activate_hovered();
                    }
                }
                // Touch has no outside-press event: a tap outside the popup is the
                // dismissal gesture, and it is not routed to this widget at all once
                // the menu is no longer topmost, so nothing further is needed here.
            }
            Event::Wheel { delta, .. } => {
                let capacity = Self::visible_capacity();
                if capacity < self.items.len() {
                    let max_offset = self.items.len() - capacity;
                    if delta.y > 0 {
                        self.scroll_offset = self.scroll_offset.saturating_sub(1);
                    } else if delta.y < 0 {
                        self.scroll_offset = (self.scroll_offset + 1).min(max_offset);
                    }
                    self.base.request_redraw();
                }
            }
            Event::KeyPress { key, modifiers } => {
                // Key codes are the framework convention (see `Key::from_key_code`): 27 Escape,
                // 13/10 Enter, 38 Up, 40 Down, 37 Left, 39 Right, 36 Home, 35 End.
                // Alt+letter is the mnemonic path; bit 2 is Alt in the event convention.
                if modifiers & 0b0100 != 0 {
                    if let Some(letter) = mnemonic_letter(*key) {
                        self.activate_mnemonic(letter);
                        return;
                    }
                }
                match *key {
                    27 => {
                        // Escape unwinds one level: an open nested panel closes before the
                        // menu itself does, which is the only order that makes Escape feel
                        // like "back" instead of "dismiss".
                        if self.close_submenu() {
                            return;
                        }
                        self.hide();
                    }
                    10 | 13 => {
                        // Enter chooses the highlighted entry. An empty highlight (or one that
                        // cannot be chosen) leaves the menu open, which is what makes a stray Enter
                        // harmless rather than a dismissal with no selection.
                        self.activate_level_hovered(self.focus_level());
                    }
                    38 => {
                        let level = self.focus_level();
                        // A grid's vertical step is one row, i.e. `columns` entries; a
                        // single-column menu keeps the historical ±1.
                        let stride = if level == 0 { self.column_count as i32 } else { 1 };
                        self.move_level_hovered(level, -stride);
                    }
                    40 => {
                        let level = self.focus_level();
                        let stride = if level == 0 { self.column_count as i32 } else { 1 };
                        self.move_level_hovered(level, stride);
                    }
                    // The horizontal pair is the only part of the navigation that follows the
                    // writing direction; see the `direction` field.
                    39 => {
                        // Right opens a branch. It only falls back to stepping the current
                        // level when the highlighted row is not a branch, so a menu of leaves
                        // keeps its original behaviour.
                        let level = self.focus_level();
                        let is_branch = self.level_hovered(level).is_some_and(|index| {
                            self.level_entries(level)
                                .and_then(|entries| entries.get(index))
                                .is_some_and(MenuEntry::has_submenu)
                        });
                        if is_branch {
                            self.activate_level_hovered(level);
                        } else if level == 0 {
                            // One entry to the right is one cell in a grid and the next row
                            // entry in a single-column list.
                            self.move_hovered_horizontally(true);
                        } else {
                            self.move_level_hovered(level, 1);
                        }
                    }
                    37 => {
                        if !self.close_submenu() {
                            self.move_hovered_horizontally(false);
                        }
                    }
                    36 => {
                        // Home/End jump to the ends of the *current* level, which is a
                        // vertical walk, so they are not mirrored either.
                        let level = self.focus_level();
                        let count = self.level_entries(level).map_or(0, |entries| entries.len());
                        self.move_level_hovered(level, -(count as i32));
                    }
                    35 => {
                        let level = self.focus_level();
                        let count = self.level_entries(level).map_or(0, |entries| entries.len());
                        self.move_level_hovered(level, count as i32);
                    }
                    // Any other key belongs to whoever opened the menu; swallowing it here would
                    // make a menu absorb an application shortcut.
                    _ => {}
                }
            }
            _ => { /* Other events are not relevant */ }
        }
    }
}
impl Draw for Menu {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        // The effect policy is read once per frame. With `menu_config` disabled this is
        // `MenuEffects::legacy()`, so every paint below takes the pre-wiring path.
        let effects = super::effects::current();

        // Chrome colours resolve the explicit style first, then the theme's resolved style for
        // this control, and only then a literal. Every colour below used to be a literal, so a
        // light/dark switch left the popup unchanged — the rendering census reported the control
        // as theme-blind.
        //
        // The theme read is a separate manager lock, taken and released inside
        // `resolved_theme_style`, so it is not held across the draw — the global manager's mutex
        // is not re-entrant.
        let style = self.base.style();
        // The **effective font** — the resolved theme/caller font — used for the heading, every
        // entry label and every shortcut, so the menu honours the theme body font and the user's
        // text scale (D09-STYLE-01).
        let font = effective_font(style);
        let theme = crate::style::resolved_theme_style_shared("menu");
        // Read as its own lock acquisition and copied out as values, so the guard is dropped
        // before anything else touches the theme.
        let (window_fill, foreground, secondary, primary) = {
            let manager = crate::style::theme_manager();
            match manager.current_theme() {
                Some(active) => (
                    active.colors.background,
                    active.colors.foreground,
                    active.colors.secondary,
                    active.colors.primary,
                ),
                None => (
                    Color::rgb(240, 240, 240),
                    Color::BLACK,
                    Color::rgb(158, 158, 158),
                    Color::rgb(33, 150, 243),
                ),
            }
        };
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(foreground);
        // `menu` is absent from `WidgetRole::for_kind_name`'s table, so it classifies as `Surface`
        // and the active theme writes the window fill into `style.background_color`. A popup
        // painted in that colour would be byte-identical to the frame behind it, so a resolved
        // surface equal to the window fill is re-derived a visible step away from it, while a
        // colour the caller set still wins.
        let face = match style.background_color {
            Some(resolved) if resolved != window_fill => resolved,
            _ => window_fill.blend(&ink, 0.08),
        };
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .filter(|resolved| *resolved != face)
            .unwrap_or_else(|| face.blend(&secondary, 0.45));

        // The heading is what makes a closed menu visible.
        //
        // A menu is born hidden, and `draw` used to `return` immediately in that state, so a
        // freshly constructed menu painted nothing and the census reported `ink = 0`. The title
        // strip is drawn in both states, which reads as the popup's heading when open and as the
        // control's own idle label when closed — and makes `title()`'s documented "drawn as its
        // heading" contract true, which it previously was not: nothing on this type drew it.
        let heading_h = self.drawn_heading_height();
        if heading_h > 0.0 {
            let heading_fill = super::effects::popup_face(face.blend(&ink, 0.10), &effects);
            context
                .fill_rect(Rect::new(rect.x, rect.y, rect.width, heading_h as u32), heading_fill);
            // The origin is the glyph's **top** edge, so the heading's vertical centre is half the
            // difference between the strip and the line box. Passing the strip's midline put the
            // glyph's top *at* the centre, so a 14 px label in a 20 px heading ended at y = 24 —
            // four pixels below the strip it belongs to.
            // The heading line height is one shaped measurement and is a function of the font
            // alone, so it is cached unconditionally. Gating the cache on `effects.caching`
            // made the default policy pay the measurement on every frame of the open reveal.
            let heading_text_h = {
                let key = font.size().to_bits();
                match self.heading_line_cache.get() {
                    Some((cached_key, height)) if cached_key == key => height,
                    _ => {
                        let height = context.measure_text("M", font).height;
                        self.heading_line_cache.set(Some((key, height)));
                        height
                    }
                }
            };
            context.draw_text(
                Point::new(rect.x + 8, rect.y + (heading_h as i32 - heading_text_h as i32) / 2),
                &self.title,
                font,
                ink,
                HorizontalAlignment::Left,
            );
            context.draw_line(
                Point::new(rect.x, rect.y + heading_h as i32),
                Point::new(rect.x + rect.width as i32, rect.y + heading_h as i32),
                border,
            );
        }

        // # Why the body is painted while it is *either* open or still closing
        //
        // `is_visible` is the authority and flips on the frame the menu opens or closes, but it is
        // a `bool` and cannot describe a panel halfway. The body is therefore drawn whenever the
        // reveal has anything left to show — which is the open state **and** the frames of a
        // closing one. Testing the flag alone would cut the closing animation off at its first
        // frame, i.e. exactly the hard cut this control was fixing.
        if !self.is_visible() && self.reveal.is_closed() {
            return;
        }

        let popup_y = rect.y + heading_h as i32;
        let popup_h = self.popup_height();
        // The body the entries are painted into, revealed from the heading downward so the menu
        // appears to grow out of its title strip. The clip is what makes the reveal honest: a
        // body that only shrank its background would still print entries outside its own edge, so
        // a user would see rows the geometry has not admitted to yet.
        let body = Rect::new(rect.x, popup_y, rect.width, popup_h as u32);
        let revealed = self.reveal.revealed(
            body,
            RevealDirection::Down,
            PopupReveal::travel_cap_for(crate::core::Size::new(body.width, body.height)),
        );
        // The shadow is painted **before** the clip: it extends outside the body by design,
        // so clipping it to the panel would hide the only part a user can see.
        if effects.shadows {
            super::effects::paint_popup_shadow(context, revealed, &effects);
        }
        // The clip is unconditional now: with scrolling a row can sit outside the body even
        // when the reveal has settled, and the clip is what keeps it invisible.
        context.push_clip(revealed.x, revealed.y, revealed.width, revealed.height);
        let body_fill = super::effects::popup_face(face.blend(&ink, 0.22), &effects);
        context.fill_rect(Rect::new(rect.x, popup_y, rect.width, popup_h as u32), body_fill);
        context.draw_rect(Rect::new(rect.x, popup_y, rect.width, popup_h as u32), border);
        // The tear-off strip is a row of dots: the drag affordance a detached menu shows at
        // its top edge. It is painted only when the caller asked for it.
        let tearoff_band = self.tearoff_band_height();
        if tearoff_band > 0 {
            let y = popup_y + tearoff_band as i32 / 2;
            let mut x = rect.x + 6;
            let right = rect.x + rect.width as i32 - 6;
            while x + 2 <= right {
                context.fill_rect(Rect::new(x, y, 2, 1), border);
                x += 4;
            }
        }
        // The run of rows, placed by the layout. `bands` is in popup coordinates, so it is offset by
        // the popup's own top edge once here rather than accumulated per row.
        let bands = self.item_bands();
        // Mnemonic labels are derived once per list/font revision and read per row (G-P06).
        let mnemonic_rows = if self.show_mnemonics { Some(self.mnemonic_rows(font)) } else { None };
        for i in self.visible_range() {
            let Some(item) = self.items.get(i) else { continue };
            let Some(row) = bands.get(i).copied() else { continue };
            let row = Rect::new(rect.x + row.x, popup_y + row.y, row.width, row.height);
            if item.is_separator() || item.is_section() {
                if item.is_separator() {
                    let sep_y = row.y + row.height as i32 / 2;
                    context.draw_line(
                        Point::new(rect.x + 4, sep_y),
                        Point::new(rect.x + rect.width as i32 - 4, sep_y),
                        border,
                    );
                } else {
                    // A section caption is a quiet label, not a choice, so it is drawn in the
                    // secondary ink and never takes the highlight fill.
                    let band =
                        Rect::new(row.x + 10, row.y, row.width.saturating_sub(20), row.height);
                    context.draw_text_fitted_cow(
                        context.text_line(band, font),
                        item.text(),
                        font,
                        secondary,
                        HorizontalAlignment::Left,
                    );
                }
                continue;
            }
            let is_hovered = self.hovered_index == Some(i);
            if is_hovered {
                context.fill_rect(
                    Rect::new(row.x + 2, row.y, row.width.saturating_sub(4), row.height),
                    primary,
                );
            }
            // A disabled entry is dimmed, and a highlighted one takes the contrast colour of the
            // primary it sits on, so neither is a fixed grey or a fixed white that only read on a
            // light popup.
            let fg = if !item.is_enabled() {
                self.base.disabled_ink_on(ink, face)
            } else if is_hovered {
                primary.contrast_color()
            } else {
                ink
            };
            // The row every text run and indicator below is placed in, and the two boxes they
            // are centred in. Deriving all three here means the check column, the label column
            // and the shortcut column are readings of one row rather than four literals, and
            // the glyph origins come from `text_line` rather than from `y + height / 2`.
            //
            // The latter was the visible defect: `draw_text`'s origin is the glyph box's
            // **top** edge, so `y + item_height() / 2` put that edge on the row's middle line
            // and drew every entry label, tick, shortcut and arrow half a line low — the same
            // error the heading directly above already documented and avoided.
            //
            // The row's own box comes from `item_bands`, so this loop no longer carries a `y`.
            let indicator = self.indicator_box(row);
            let label = self.label_box(row, indicator);
            if let Some(source) = item.icon() {
                // An icon occupies the indicator column, so a checkable row that also carries
                // an icon shows the icon rather than two marks competing for one box.
                let line = context.text_line(indicator, font);
                let side = line.height.max(1).min(indicator.width.max(1));
                let icon_rect = Rect::new(
                    indicator.x + (indicator.width as i32 - side as i32) / 2,
                    line.y,
                    side,
                    side,
                );
                if let Err(error) = crate::widget::paint_icon(context, source.into(), icon_rect, fg)
                {
                    log::warn!("menu entry icon could not be painted: {error:?}");
                }
            } else if item.is_checkable() {
                // The tick is the `Check` **icon outline**, not the `✓` text glyph (U+2713 is
                // covered by no bundled face, so it drew as an 8x8 bitmap block). An unchecked row
                // draws nothing, which is what the old blank-space string achieved.
                if item.is_checked() {
                    let line = context.text_line(indicator, font);
                    let side = line.height.max(1);
                    let tick = Rect::new(
                        indicator.x + (indicator.width as i32 - side as i32) / 2,
                        line.y,
                        side,
                        side,
                    );
                    crate::widget::draw_icon_at(context, tick, fg, IconName::Check);
                }
            }
            // Mnemonic markers are metadata until the caller asks for them: with the flag
            // off the raw label is drawn, byte for byte as before.
            let label_text = match mnemonic_rows.as_ref().and_then(|rows| rows.get(i)) {
                Some(row) => row.text.as_str(),
                None => item.text(),
            };
            let line = context.text_line(label, font);
            context.draw_text_fitted_cow(line, label_text, font, fg, HorizontalAlignment::Left);
            if let Some((_, prefix, width)) =
                mnemonic_rows.as_ref().and_then(|rows| rows.get(i)).and_then(|row| row.underline)
            {
                // The underline is placed from the **cached** measured prefix, so it tracks the
                // glyph the marker names without re-measuring the label on every frame.
                let underline = Rect::new(
                    line.x + prefix as i32,
                    line.y + line.height as i32 - 1,
                    width.max(1),
                    1,
                );
                context.fill_rect(underline, fg);
            }
            if self.row_shows_shortcut(item) {
                // One line box for the shortcut and the arrow: they occupy one trailing column,
                // so they cannot disagree about where that column's midline is — and only one of
                // them is ever drawn in it (`row_shows_shortcut`).
                let trailing = Rect::new(
                    self.trailing_column_x(row),
                    row.y,
                    self.trailing_width(),
                    row.height,
                );
                context.draw_text_fitted_cow(
                    context.text_line(trailing, font),
                    item.shortcut(),
                    font,
                    fg,
                    HorizontalAlignment::Right,
                );
            }
            if item.has_submenu() {
                // Derived from the label box's own right edge (see `submenu_arrow_box`), so a longer
                // label shortens the room the arrow has instead of the two being placed from opposite
                // ends of the row and colliding in the middle. The mark is the `ChevronRight` **icon
                // outline**, not the `▶` text glyph (U+25B6 is covered by no bundled face, so it drew
                // as an 8x8 bitmap block).
                let arrow_box = self.submenu_arrow_box(row, label);
                let side = arrow_box.width.min(arrow_box.height).min(16);
                crate::widget::draw_icon_centered(
                    context,
                    arrow_box,
                    side,
                    fg,
                    IconName::ChevronRight,
                );
            }
        }
        // Popped on the same path it was pushed, so the reveal cannot leak a clip onto whatever
        // the host draws after this menu.
        context.pop_clip();
        // The nested panel is painted after the parent's clip is popped: it hangs outside the
        // parent rectangle by design, so clipping it to the parent would hide all of it.
        if self.is_visible() || !self.reveal.is_closed() {
            self.draw_submenu_panel(context, font, ink, secondary, primary, border, face);
        }

        // The hover hint is drawn last, over the row it explains and any open child panel.
        let geometry = self.geometry();
        self.tooltips.set_geometry(geometry);
        self.tooltips.draw(context);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::svg::render_to_svg;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    /// One ink box per text run in document order, as `(left, top, right, bottom)`.
    ///
    /// # Why the ink and not the string
    ///
    /// Text leaves the SVG backend as glyph geometry, not a `<text>` element, so the label is not
    /// in the document in any form and a test has to locate a run by *where* it is. That is the
    /// stronger check: the old form matched `>Open</text>` and read the element's `y`, so a glyph
    /// placed a line away with a correct attribute would have passed it.
    ///
    /// # Why the boxes come from the crate reader, not a local `d` parser
    ///
    /// This used to split each `d` on `M` and read four integers as `x y w h` — the **bitmap**
    /// path's grammar and only its grammar. An outline face writes `M17.00 23.84L...Z` with
    /// fractional vertices, which that reader took as a rectangle at `(17, 23)` sized by the next
    /// two numbers: geometry that is not ink and not where the glyph is, so the labels were located
    /// off their rows. `text_ink_boxes` knows both grammars and the `data-text` provenance tag; one
    /// reader, so the two cannot drift apart.
    fn text_run_boxes(svg: &str) -> Vec<(i32, i32, i32, i32)> {
        crate::widget::svg::text_ink_boxes(svg)
    }

    /// The bitmap face fills its glyph box, so its ink top *is* the box top. An outline face draws a
    /// real glyph whose ink is inset, so the ink top sits one or two pixels below the box top at these
    /// sizes. Pinning the two equal encoded a property of the bitmap face, not of the layout.
    const INK_INSET_TOLERANCE: i32 = 3;

    /// A menu row's label is centred on its own line box, not offset by half a line.
    ///
    /// `draw_text`'s origin is the glyph box's **top** edge, so the previous
    /// `y + item_height() / 2` origin put that edge on the row's middle line and drew every
    /// entry half a line low — the same mistake the heading above already avoids. Measured
    /// through the rendered SVG so the assertion is about what a person sees.
    #[test]
    fn a_menu_row_is_centred_on_its_own_line_box() {
        // Holds the crate-wide theme guard: this test renders, and a concurrent
        // test that switches the appearance would otherwise change a later frame.
        let _theme_guard = crate::style::theme_test_guard();
        let mut menu = Menu::new("File", Rect::new(0, 0, 200, 120));
        menu.add_action("Open");
        menu.add_action("Save");
        // A menu is a popup and is born hidden, so its rows are only drawn once opened.
        menu.open_at(Point::new(0, 0), Rect::new(0, 0, 1000, 800));
        let svg = crate::widget::svg::render_to_svg(&mut menu);

        // The popup starts below the 20 px heading and 2 px into the body, so the first row is
        // at y = 22 and is 22 px tall. A 14 px line box centred in it starts at 22 + (22-14)/2.
        let row_top = Menu::heading_height() as i32 + 2;
        let line_h = {
            let mut backend = crate::render::SvgPaintBackend::new(crate::core::Size::new(200, 120));
            crate::render::RenderContext::new(&mut backend)
                .measure_text("M", &crate::core::Font::default())
                .height as i32
        };
        let expected_y = row_top + (Menu::item_height() as i32 - line_h) / 2;
        // The rows are located by the band they occupy, not by the string they spell: a run
        // belongs to a row when its ink's top edge falls inside that row's own band. Every label
        // ('O', 'S', 'F') lights its glyph's top row, so the ink top is the glyph box's top edge
        // under the bitmap face and within the inset of it under an outline face — inside the
        // 22 px row either way, so the band test separates the rows.
        let runs = text_run_boxes(&svg);
        let on_row = |index: usize| -> (i32, i32, i32, i32) {
            let top = row_top + index as i32 * Menu::item_height() as i32;
            runs.iter()
                .find(|(_, t, _, _)| *t >= top && *t < top + Menu::item_height() as i32)
                .copied()
                .unwrap_or_else(|| panic!("row {index} painted no ink (rows at {top})"))
        };
        let first = on_row(0);
        // The glyph box's top edge is the line box; the ink top is that edge under the bitmap face
        // and one or two pixels in under an outline face, so the two are pinned equal to within the
        // inset rather than exactly. A row still avoids the old `+ item_height() / 2` origin — that
        // mistake is four pixels out here, past the tolerance.
        assert!(
            (first.1 - expected_y).abs() <= INK_INSET_TOLERANCE,
            "the label belongs on its row's line box: ink top {}, box top {expected_y}",
            first.1
        );
        // Every row derives its origin the same way, so the second row's line box is the first's
        // shifted down by exactly one row.
        assert!(
            (on_row(1).1 - (expected_y + Menu::item_height() as i32)).abs() <= INK_INSET_TOLERANCE,
            "and every row derives its own line box the same way: ink top {}, box top {}",
            on_row(1).1,
            expected_y + Menu::item_height() as i32
        );
        assert_ne!(
            first.1,
            row_top + Menu::item_height() as i32 / 2,
            "the row's middle is not a glyph-box top edge"
        );
        assert!(first.2 > first.0, "the label laid down ink: {first:?}");
        // The ink must stay inside the row it labels, which the old attribute check could not
        // see: the label column is inset by the indicator, so it cannot start at the row's edge.
        assert!(first.0 > 0 && first.2 <= 200, "the label stays within the popup: {first:?}");
    }

    /// The label's column is whatever the indicator and the trailing column leave.
    ///
    /// BLUE22 §B.8 lists this control for exactly this: the check column, the label column and
    /// the two trailing columns were four independent literals (`x + 8`, `x + 28`, `right - 8`,
    /// `right - 4`), so a wider indicator or a longer shortcut did not *push* its neighbour.
    /// The standard table states the relation directly.
    #[test]
    fn a_row_reserves_its_indicator_and_trailing_columns() {
        let menu = Menu::new("File", Rect::new(0, 0, 200, 120));
        let row = Rect::new(0, 22, 200, Menu::item_height() as u32);
        let indicator = menu.indicator_box(row);
        let label = menu.label_box(row, indicator);

        assert_eq!(
            indicator.x - row.x,
            dimensions::MENU_ROW_PADDING_H as i32,
            "the indicator uses the row's own padding"
        );
        assert_eq!(
            label.x,
            indicator.x + indicator.width as i32 + dimensions::INDICATOR_TEXT_SPACING as i32,
            "the label's column is derived from the indicator's box"
        );
        assert_eq!(
            label.x + label.width as i32,
            row.x + row.width as i32 - dimensions::MENU_ROW_TRAILING_WIDTH as i32,
            "the label yields to the shortcut/arrow column"
        );
    }

    /// blue27 §19.13 case I12: the columns a menu row actually draws are disjoint.
    ///
    /// One row can carry a **check mark** (the indicator column), a label, a **shortcut** and a
    /// **submenu arrow** — three of the four are icon slots, so they are exactly the boxes the unified
    /// contract must keep apart, and the failure mode is silent: two boxes can overlap while every
    /// individual assertion about them still passes. The check is therefore pairwise on the drawn set.
    ///
    /// The trailing column is shared, so "at most one of shortcut/arrow" is part of the contract rather
    /// than an accident: this test found the two being drawn in the *same* box before
    /// [`Menu::row_shows_shortcut`] stated the precedence.
    #[test]
    fn a_rows_drawn_columns_are_disjoint_even_when_the_row_is_narrow() {
        let menu = Menu::new("File", Rect::new(0, 0, 120, 120));
        // Narrow enough that label + shortcut + arrow genuinely compete for the same pixels.
        let row = Rect::new(0, 22, 120, Menu::item_height() as u32);

        let with_shortcut = MenuEntry::with_shortcut(MenuEntry::new("Toggle"), "Ctrl+T");
        let with_arrow = {
            let mut entry = MenuEntry::new("More");
            entry.set_has_submenu(true);
            entry
        };
        let with_both = {
            let mut entry = MenuEntry::with_shortcut(MenuEntry::new("Both"), "Ctrl+B");
            entry.set_has_submenu(true);
            entry
        };

        // A row shows its shortcut only when it has no submenu, so the drawn set is at most three
        // boxes and never the two trailing affordances at once.
        assert!(menu.row_shows_shortcut(&with_shortcut));
        assert!(!menu.row_shows_shortcut(&with_arrow));
        assert!(
            !menu.row_shows_shortcut(&with_both),
            "the arrow takes the shared trailing column; a shortcut there would overlap it"
        );

        let indicator = menu.indicator_box(row);
        let label = menu.label_box(row, indicator);
        let arrow = menu.submenu_arrow_box(row, label);
        let shortcut = Rect::new(
            menu.trailing_column_x(row),
            row.y,
            dimensions::MENU_ROW_TRAILING_WIDTH,
            row.height,
        );

        for (name, entry) in
            [("shortcut-only", &with_shortcut), ("submenu", &with_arrow), ("both", &with_both)]
        {
            let mut drawn: Vec<(&str, Rect)> = vec![("indicator", indicator), ("label", label)];
            if menu.row_shows_shortcut(entry) {
                drawn.push(("shortcut", shortcut));
            }
            if entry.has_submenu() {
                drawn.push(("arrow", arrow));
            }
            for (i, (box_name, box_)) in drawn.iter().enumerate() {
                assert!(
                    box_.width > 0,
                    "{name}: {box_name} collapsed to zero width, so nothing was asserted about it"
                );
                for (other_name, other) in &drawn[i + 1..] {
                    let disjoint = box_.x + box_.width as i32 <= other.x
                        || other.x + other.width as i32 <= box_.x;
                    assert!(
                        disjoint,
                        "{name}: {box_name} {box_:?} overlaps {other_name} {other:?}; the drawn \
                         columns must partition the row, not be derived independently"
                    );
                }
            }
        }

        // ...and they are in reading order, so "disjoint" cannot be satisfied by a permutation, and
        // every drawn box stays inside the row.
        assert!(indicator.x < label.x && label.x < shortcut.x);
        assert!(shortcut.x <= arrow.x, "the arrow follows the shortcut column");
        for (name, box_) in [("label", label), ("shortcut", shortcut), ("arrow", arrow)] {
            assert!(
                box_.x >= row.x && box_.x + box_.width as i32 <= row.x + row.width as i32,
                "{name} {box_:?} escapes the row {row:?}"
            );
        }
    }

    /// The check mark and the submenu arrow reach the screen as **geometry**, not as characters.
    ///
    /// §19.11 requires an indicator not to add a duplicate node a screen reader would announce, and
    /// the crate reached this by drawing `IconName::Check` / `IconName::ChevronRight` outlines instead
    /// of the `✓` / `▶` characters no bundled face covers. Asserted on the document because that is
    /// what a reader consumes: the mark must be an icon fill, and neither character may appear.
    #[test]
    fn the_check_and_arrow_are_geometry_not_text() {
        let mut menu = Menu::new("Edit", Rect::new(0, 0, 200, 120));
        let mut entry = MenuEntry::new("Toggle");
        entry.set_checkable(true);
        entry.set_checked(true);
        entry.set_has_submenu(true);
        entry.set_shortcut("Ctrl+T");
        menu.add_item(entry);

        let svg = render_to_svg(&mut menu);
        assert!(
            svg.contains(r#"fill-rule="nonzero""#),
            "the tick and the arrow must arrive as icon geometry"
        );
        for character in ['\u{2713}', '\u{25B6}'] {
            assert!(
                !svg.contains(character),
                "the indicator must not be the {character:?} character: it is covered by no bundled \
                 face and would read as a second node"
            );
        }
    }

    /// A wider indicator narrows the label rather than overlapping it (the §B.9 property).
    #[test]
    fn a_wider_indicator_pushes_the_label_rather_than_overlapping_it() {
        let menu = Menu::new("File", Rect::new(0, 0, 200, 120));
        let short_row = Rect::new(0, 0, 200, dimensions::CHECKBOX_BOX);
        let tall_row = Rect::new(0, 0, 200, dimensions::CHECKBOX_BOX * 2);
        let short = menu.label_box(short_row, menu.indicator_box(short_row));
        let tall = menu.label_box(tall_row, menu.indicator_box(tall_row));
        assert_eq!(
            short.x, tall.x,
            "the indicator column's width is fixed, so the label's start does not move"
        );
        assert!(tall.x + tall.width as i32 <= tall_row.x + tall_row.width as i32);
        assert!(short.x > short_row.x, "the label never starts at the row's own edge");
    }

    #[test]
    fn menu_item_state_accessors_handle_valid_and_oob_indices() {
        let mut menu = Menu::new("File", Rect::new(0, 0, 200, 120));
        let idx = menu.add_action("Open");

        assert_eq!(menu.item_enabled(idx), Some(true));
        menu.set_item_enabled(idx, false);
        assert_eq!(menu.item_enabled(idx), Some(false));
        assert_eq!(menu.item_enabled(99), None);

        menu.items[idx].set_checkable(true);
        assert_eq!(menu.item_checked(idx), Some(false));
        menu.set_item_checked(idx, true);
        assert_eq!(menu.item_checked(idx), Some(true));
        assert_eq!(menu.item_checked(99), None);
    }

    fn context_menu() -> Menu {
        let mut menu = Menu::new("Edit", Rect::new(0, 0, 160, 100));
        menu.add_action("Cut");
        menu.add_action("Copy");
        menu.add_separator();
        menu.add_action("Paste");
        menu
    }

    /// A context menu must appear where the pointer was, and report that origin.
    #[test]
    fn open_at_places_the_popup_at_the_pointer() {
        let mut menu = context_menu();
        // A menu is born hidden: it is a popup, not a panel, so `open_at` is what
        // makes it appear. Asserting that first keeps the test from passing on a
        // menu that was already visible for an unrelated reason.
        assert!(!menu.is_visible(), "a menu must start hidden");

        menu.open_at(Point::new(300, 200), Rect::new(0, 0, 1000, 800));

        assert!(menu.is_visible());
        assert_eq!(menu.invoker_position(), Some(Point::new(300, 200)));
        assert_eq!(menu.geometry().x, 300);
        assert_eq!(menu.geometry().y, 200);
    }

    /// Opened near an edge, the popup must shift so it stays fully on screen —
    /// otherwise part of the menu is unreachable.
    #[test]
    fn open_at_clamps_a_popup_that_would_overflow() {
        let viewport = Rect::new(0, 0, 200, 150);
        let mut menu = context_menu();

        menu.open_at(Point::new(195, 148), viewport);

        let rect = menu.geometry();
        assert!(
            rect.x + rect.width as i32 <= viewport.width as i32,
            "popup must not extend past the right edge: {rect:?}",
        );
        assert!(
            rect.y + menu.popup_height() as i32 <= viewport.height as i32,
            "popup must not extend past the bottom edge",
        );
        // The invoker position is recorded unclamped: it describes where the user
        // clicked, which is a different fact from where the popup ended up.
        assert_eq!(menu.invoker_position(), Some(Point::new(195, 148)));
    }

    /// A press inside the popup that hits an entry triggers it and closes the menu.
    #[test]
    fn pressing_an_entry_triggers_it_and_closes_the_menu() {
        use std::sync::{Arc, Mutex};

        let mut menu = context_menu();
        menu.open_at(Point::new(10, 10), Rect::new(0, 0, 400, 400));
        // Signal slots are `Send + Sync`, so the sink is an `Arc<Mutex<_>>` rather
        // than the `Rc<RefCell<_>>` a single-threaded test would otherwise use.
        let fired = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&fired);
        menu.triggered.connect(move |text: Arc<String>| {
            sink.lock().expect("sink poisoned").push((*text).clone());
        });

        // The first entry is drawn below the heading strip and the body's 2px top
        // padding. The test used to click at `y = 14`, inside the heading — it passed
        // only because the hit path started its row walk at the popup's top edge while
        // `draw` started it below the heading, i.e. because the two disagreed.
        let body_top = menu.body_top();
        menu.handle_event(&Event::MousePress {
            pos: Point::new(20, body_top + 4),
            button: 1,
            modifiers: 0,
        });

        assert_eq!(&*fired.lock().expect("sink poisoned"), &["Cut".to_string()]);
        assert!(!menu.is_visible(), "choosing an entry must close the menu");
    }

    /// The dismissal gesture people actually use is a left-click elsewhere, so an
    /// outside press must close the menu whichever button was used.
    #[test]
    fn a_press_outside_the_popup_closes_it_with_either_button() {
        for button in [crate::event::mouse_button::PRIMARY, crate::event::mouse_button::SECONDARY] {
            let mut menu = context_menu();
            menu.open_at(Point::new(100, 100), Rect::new(0, 0, 400, 400));
            assert!(menu.is_visible());

            menu.handle_event(&Event::MousePress { pos: Point::new(5, 5), button, modifiers: 0 });

            assert!(!menu.is_visible(), "button {button} outside the popup must dismiss it");
        }
    }

    /// A press on the popup's own padding is still "inside": it must not dismiss,
    /// or clicking a menu's edge would close it before an entry could be chosen.
    #[test]
    fn a_press_inside_the_popup_does_not_dismiss_it() {
        let mut menu = context_menu();
        menu.open_at(Point::new(100, 100), Rect::new(0, 0, 400, 400));

        // x is inside the popup but past the text column; y is the top padding.
        menu.handle_event(&Event::MousePress {
            pos: Point::new(250, 101),
            button: 1,
            modifiers: 0,
        });

        assert!(menu.is_visible(), "padding inside the popup must not dismiss the menu");
    }

    /// Reopening must not report the previous invocation's position.
    #[test]
    fn hiding_clears_the_invoker_position() {
        let mut menu = context_menu();
        menu.open_at(Point::new(10, 10), Rect::new(0, 0, 400, 400));
        assert!(menu.invoker_position().is_some());

        menu.hide();

        assert_eq!(menu.invoker_position(), None);
        assert_eq!(menu.hovered_index(), None);
    }

    /// The popup's hit area follows its drawn height, not the stored geometry
    /// height, so the outside test agrees with what the user sees.
    #[test]
    fn contains_point_follows_the_drawn_height() {
        let mut menu = context_menu();
        // Geometry height is 100 but the four entries plus padding draw shorter, and the
        // heading sits above them.
        menu.open_at(Point::new(0, 0), Rect::new(0, 0, 400, 400));

        let drawn = menu.body_top() + menu.popup_height() as i32;
        assert!(menu.contains_point(Point::new(5, drawn - 1)));
        assert!(!menu.contains_point(Point::new(5, drawn)));
    }

    /// The rows tile the popup in sequence, below its own padding.
    ///
    /// # What this pins
    ///
    /// BLUE22 §B.8 lists this control's defect as "the check and arrow paddings written by hand".
    /// The columns inside a row were already derived; what this pins is the **run**: each row starts
    /// where the previous one ended, separators get `MENU_SEPARATOR_HEIGHT` and entries
    /// `MENU_ROW_HEIGHT`, and the first row starts `MENU_POPUP_PADDING` below the popup's top edge.
    /// The old form carried that arithmetic as a `let mut y` inside the painting loop, so nothing
    /// could ask for a row's box without painting the menu.
    #[test]
    fn the_rows_tile_the_popup_in_sequence() {
        let mut menu = Menu::new("File", Rect::new(0, 0, 200, 120));
        menu.add_action("Open");
        menu.add_separator();
        menu.add_action("Save");

        let bands = menu.item_bands();
        assert_eq!(bands.len(), 3, "one band per entry, separators included");
        assert_eq!(
            bands[0].y,
            dimensions::MENU_POPUP_PADDING as i32,
            "the first row starts below the popup's own padding: {bands:?}"
        );
        for pair in bands.windows(2) {
            assert_eq!(
                pair[1].y,
                pair[0].y + pair[0].height as i32,
                "each row follows the previous one's own box: {bands:?}"
            );
        }
        assert_eq!(bands[0].height, Menu::item_height() as u32);
        assert_eq!(bands[1].height, Menu::separator_height() as u32);
        assert_eq!(bands[2].height, Menu::item_height() as u32);
        // The run is exactly as tall as the popup the control reports.
        let last = bands.last().expect("three bands");
        assert_eq!(
            last.y + last.height as i32 + dimensions::MENU_POPUP_PADDING as i32,
            menu.popup_height() as i32,
            "the rows and the popup's padding add up to the popup's own height"
        );
    }

    /// A separator sits between two entries and does not disturb their alignment.
    ///
    /// The companion invariant to the run: the columns of a row are read from the row's own box, so
    /// a separator shifting every later row down must not shift their *labels* sideways.
    #[test]
    fn a_separator_shifts_the_rows_but_not_their_columns() {
        let mut menu = Menu::new("File", Rect::new(0, 0, 200, 120));
        menu.add_action("Open");
        menu.add_action("Save");
        let without: Vec<i32> = menu
            .item_bands()
            .iter()
            .map(|row| menu.label_box(*row, menu.indicator_box(*row)).x)
            .collect();

        menu.add_separator();
        let with: Vec<i32> = menu
            .item_bands()
            .iter()
            .take(2)
            .map(|row| menu.label_box(*row, menu.indicator_box(*row)).x)
            .collect();
        assert_eq!(without, with, "a separator must not move the label columns");
    }

    /// The highlight must walk the entry list, wrap at both ends, and step over separators.
    ///
    /// # The defect this pins
    ///
    /// The menu handled Escape and Enter and nothing else, so every entry past the first was
    /// unreachable without a mouse even though `open_at` pre-selects one "so keyboard navigation has
    /// a starting point". A selection that no key can move is an affordance that is declared and not
    /// delivered.
    #[test]
    fn arrow_keys_walk_the_entries_and_skip_separators() {
        let mut menu = context_menu();
        menu.open_at(Point::new(10, 10), Rect::new(0, 0, 400, 400));
        // `open_at` pre-selects the first actionable entry.
        assert_eq!(menu.hovered_index(), Some(0));

        // Down: 0 -> 1 -> (separator skipped) -> 3.
        menu.handle_event(&Event::KeyPress { key: 40, modifiers: 0 });
        assert_eq!(menu.hovered_index(), Some(1), "Down must move to the next entry");
        menu.handle_event(&Event::KeyPress { key: 40, modifiers: 0 });
        assert_eq!(menu.hovered_index(), Some(3), "Down must step over the separator");
        // Down again wraps to the top.
        menu.handle_event(&Event::KeyPress { key: 40, modifiers: 0 });
        assert_eq!(menu.hovered_index(), Some(0), "Down past the end must wrap");

        // Up wraps the other way, again without landing on the separator.
        menu.handle_event(&Event::KeyPress { key: 38, modifiers: 0 });
        assert_eq!(menu.hovered_index(), Some(3), "Up past the start must wrap to the last entry");
        assert!(menu.is_visible(), "navigating must not dismiss the menu");
    }

    /// A disabled entry is not a destination: the highlight must step over it too.
    ///
    /// Stopping *on* one would leave Enter doing nothing with no visible explanation, which reads as a
    /// frozen menu rather than a disabled entry.
    #[test]
    fn arrow_keys_skip_disabled_entries() {
        let mut menu = Menu::new("Edit", Rect::new(0, 0, 160, 100));
        menu.add_action("Cut");
        let disabled = menu.add_action("Copy");
        menu.add_action("Paste");
        menu.set_item_enabled(disabled, false);
        menu.open_at(Point::new(0, 0), Rect::new(0, 0, 400, 400));

        assert_eq!(menu.hovered_index(), Some(0));
        menu.handle_event(&Event::KeyPress { key: 40, modifiers: 0 });
        assert_eq!(menu.hovered_index(), Some(2), "Down must step over the disabled entry");
        menu.handle_event(&Event::KeyPress { key: 38, modifiers: 0 });
        assert_eq!(menu.hovered_index(), Some(0), "and Up must step back over it");
    }

    /// Enter chooses the highlighted entry; a menu with nothing choosable stays open.
    #[test]
    fn enter_activates_the_highlight_and_leaves_an_empty_menu_open() {
        let mut menu = context_menu();
        menu.open_at(Point::new(0, 0), Rect::new(0, 0, 400, 400));
        menu.handle_event(&Event::KeyPress { key: 40, modifiers: 0 });

        let chosen = Arc::new(Mutex::new(Vec::<usize>::new()));
        let sink = chosen.clone();
        menu.triggered_index.connect(move |index| {
            sink.lock().unwrap().push(*index);
        });
        menu.handle_event(&Event::KeyPress { key: 13, modifiers: 0 });
        assert_eq!(*chosen.lock().unwrap(), vec![1], "Enter must trigger the highlighted entry");
        assert!(!menu.is_visible(), "choosing an entry must dismiss the menu");

        // Nothing highlighted: Enter is inert rather than a dismissal with no selection.
        let mut empty = Menu::new("Edit", Rect::new(0, 0, 160, 100));
        empty.add_separator();
        empty.open_at(Point::new(0, 0), Rect::new(0, 0, 400, 400));
        assert_eq!(empty.hovered_index(), None, "a separator is not a starting point");
        empty.handle_event(&Event::KeyPress { key: 13, modifiers: 0 });
        assert!(empty.is_visible(), "Enter with no highlight must leave the menu open");
    }

    /// Right/Left follow the writing direction; Up/Down do not.
    ///
    /// A list of entries runs downward in every locale, but the horizontal pair steps along a line of
    /// reading order — and a menu is the child of a menu bar, where that pair is what moves between a
    /// bar's drop-downs and into a submenu. Mirroring the vertical pair instead would make an Arabic
    /// menu's Up key walk backwards.
    #[test]
    fn horizontal_arrows_follow_the_direction_and_vertical_ones_do_not() {
        for (direction, right_is_forward) in [
            (crate::core::TextDirection::LeftToRight, true),
            (crate::core::TextDirection::RightToLeft, false),
        ] {
            let mut menu = context_menu();
            menu.set_direction(direction);
            menu.open_at(Point::new(0, 0), Rect::new(0, 0, 400, 400));
            assert_eq!(menu.hovered_index(), Some(0));

            // Key 39 is Right, key 37 is Left.
            let (key, expected) = if right_is_forward { (39, Some(1)) } else { (39, Some(3)) };
            menu.handle_event(&Event::KeyPress { key, modifiers: 0 });
            assert_eq!(
                menu.hovered_index(),
                expected,
                "{direction:?}: Right must move {}",
                if right_is_forward { "forward" } else { "backward" }
            );

            // The vertical pair is untouched by the direction: Down always advances a row.
            let mut vertical = context_menu();
            vertical.set_direction(direction);
            vertical.open_at(Point::new(0, 0), Rect::new(0, 0, 400, 400));
            vertical.handle_event(&Event::KeyPress { key: 40, modifiers: 0 });
            assert_eq!(
                vertical.hovered_index(),
                Some(1),
                "{direction:?}: Down must advance a row in every locale"
            );
        }
    }

    /// The default must be byte-identical, so a menu that never asks behaves exactly as it did.
    #[test]
    fn the_default_menu_is_still_left_to_right() {
        // The two renders are compared to each other, so the appearance has to be the same for both
        // halves: a concurrent theme switch between them would make this fail for a reason that has
        // nothing to do with direction.
        let _guard = crate::style::theme_test_guard();
        let mut untouched = context_menu();
        untouched.open_at(Point::new(0, 0), Rect::new(0, 0, 400, 400));
        let mut ltr = context_menu();
        ltr.set_direction(crate::core::TextDirection::LeftToRight);
        ltr.open_at(Point::new(0, 0), Rect::new(0, 0, 400, 400));
        assert_eq!(render_to_svg(&mut untouched), render_to_svg(&mut ltr));
    }

    /// The direction is reachable from the property API with a matching read-back token.
    #[test]
    fn the_direction_round_trips_through_the_property_api() {
        let mut menu = context_menu();
        assert_eq!(menu.get("direction").unwrap().as_str(), Some("ltr"));
        menu.set("direction", CapabilityValue::String("rtl".to_string())).unwrap();
        assert_eq!(menu.direction(), crate::core::TextDirection::RightToLeft);
        assert_eq!(menu.get("direction").unwrap().as_str(), Some("rtl"));
    }

    // ── G-3: a submenu parent is a branch, not a leaf ──────────────────

    /// Choosing an entry that owns a submenu must ask for the submenu, never run the entry.
    ///
    /// # The defect this closes (BLUE22 · G-3)
    ///
    /// `MenuEntry::has_submenu` had exactly one consumer: the draw path, which paints the arrow.
    /// Both activation paths ignored it, so a click or an Enter on `Cut >` emitted `triggered`
    /// with `"Cut"` and hid the popup — the user got a command they had not chosen and the
    /// submenu was unreachable by pointer at all. Every test that drove a *leaf* entry passed
    /// either way, which is why this needs a test written specifically for the branch.
    #[test]
    fn a_submenu_parent_is_a_branch_and_not_a_leaf() {
        // A menu built for this test: `context_menu()` already carries four entries, so adding to
        // it would put the branch on row 4 and the press below would land on a leaf.
        let mut menu = Menu::new("Edit", Rect::new(0, 0, 160, 100));
        let mut parent = MenuEntry::new("Cut");
        parent.set_has_submenu(true);
        menu.add_item(parent);
        menu.add_item(MenuEntry::new("Copy"));
        menu.open_at(Point::new(0, 0), Rect::new(0, 0, 400, 400));

        let opened = Arc::new(AtomicUsize::new(0));
        let chosen = Arc::new(AtomicUsize::new(0));
        let opened_clone = Arc::clone(&opened);
        let chosen_clone = Arc::clone(&chosen);
        menu.submenu_requested.connect(move |_: Arc<String>| {
            opened_clone.fetch_add(1, Ordering::SeqCst);
        });
        menu.triggered.connect(move |_: Arc<String>| {
            chosen_clone.fetch_add(1, Ordering::SeqCst);
        });

        // A press on the first row, which is the branch. The popup and its rows are drawn
        // below the heading, so the press uses the same `body_top` derivation the painter does.
        let row_y = menu.body_top() + 4;
        menu.handle_event(&Event::MousePress {
            pos: Point::new(20, row_y),
            button: 1,
            modifiers: 0,
        });

        assert_eq!(opened.load(Ordering::SeqCst), 1, "the branch must be reported as opened");
        assert_eq!(
            chosen.load(Ordering::SeqCst),
            0,
            "a branch must never report a committed choice -- that is the defect"
        );
        assert!(menu.is_visible(), "a branch that opened must leave the popup up, not hide it");
    }

    /// A leaf entry still commits exactly as before, so the branch rule did not swallow it.
    ///
    /// The dual of the test above, and the reason the rule is stated as "submenu parent" rather
    /// than "any entry": without this, guarding both paths could have made *every* click a
    /// branch and the menu would never choose anything.
    #[test]
    fn a_leaf_entry_commits_and_hides_the_menu() {
        let mut menu = Menu::new("Edit", Rect::new(0, 0, 160, 100));
        menu.add_item(MenuEntry::new("Copy"));
        menu.open_at(Point::new(0, 0), Rect::new(0, 0, 400, 400));

        let opened = Arc::new(AtomicUsize::new(0));
        let chosen = Arc::new(AtomicUsize::new(0));
        let opened_clone = Arc::clone(&opened);
        let chosen_clone = Arc::clone(&chosen);
        menu.submenu_requested.connect(move |_: Arc<String>| {
            opened_clone.fetch_add(1, Ordering::SeqCst);
        });
        menu.triggered.connect(move |_: Arc<String>| {
            chosen_clone.fetch_add(1, Ordering::SeqCst);
        });

        menu.handle_event(&Event::MousePress {
            pos: Point::new(20, menu.body_top() + 4),
            button: 1,
            modifiers: 0,
        });

        assert_eq!(chosen.load(Ordering::SeqCst), 1, "a leaf must commit");
        assert_eq!(opened.load(Ordering::SeqCst), 0, "a leaf must not ask for a submenu");
        assert!(!menu.is_visible(), "committing a leaf must dismiss the popup");
    }

    /// The submenu request is published, so a `forward_all` subscriber can reach it.
    ///
    /// An event that fires but is absent from the capability's list is unreachable through
    /// `connect_event` — the designer would offer no way to subscribe to it. This reads the
    /// capability the way a host does, through the factory, rather than from the string table:
    /// that is the same answer the event panel and `forward_all` get.
    #[test]
    fn the_submenu_event_is_published_by_the_capability() {
        let factory = crate::widget::WidgetFactory::shared();
        let menu = Menu::new("File", Rect::new(0, 0, 200, 200));
        let capability =
            factory.capability_for_kind_instance(&menu).expect("a menu resolves a capability");
        assert!(
            capability.events.iter().any(|event| event.name == "submenu_requested"),
            "an emitted event must be published: {:?}",
            capability.events.iter().map(|e| e.name).collect::<Vec<_>>()
        );
    }

    // ── D09-STYLE-01: the menu consumes the resolved theme/caller font ──

    /// A caller-authored `style.font` must change the heading and entry rows the menu paints.
    ///
    /// # The defect this pins
    ///
    /// The heading and every row label built a `Font::default()` by hand, so the theme body font
    /// and the user's text scale never reached a menu. They now read `metrics::effective_font`.
    #[test]
    fn a_custom_font_changes_the_menu_ink_width() {
        let render = |font: Option<crate::core::Font>| {
            let mut menu = Menu::new("File", Rect::new(0, 0, 260, 140));
            menu.add_action("Open");
            if let Some(font) = font {
                menu.set_style(crate::style::WidgetStyle::default().with_font(font));
            }
            menu.open_at(Point::new(0, 0), Rect::new(0, 0, 1000, 800));
            let svg = crate::widget::svg::render_to_svg(&mut menu);
            crate::widget::svg::text_ink_box(&svg).map(|(x, _, right, _)| right - x).unwrap_or(0)
        };

        let base_ink = render(None);
        let big_ink = render(Some(crate::core::Font::simple("Test", 30.0)));
        assert!(base_ink > 0, "the default menu must paint ink for this test to mean anything");
        assert!(
            big_ink > base_ink,
            "a larger font must widen the painted menu ink: {big_ink} vs {base_ink}"
        );
    }

    /// The open reveal must actually reach its end, or the popup stays a slit.
    ///
    /// This is the model half of the defect the GTK demo showed: the first screenshot after a
    /// click had the heading and a sliver of body, which is exactly `reveal < 1`. Isolating the
    /// tick here says whether the value arrives (a driver problem) or the value is right and the
    /// painter/frame loop is not (a repaint problem).
    #[test]
    fn the_open_reveal_settles_after_a_second_of_frames() {
        let mut menu = context_menu();
        menu.open_at(Point::new(0, 0), Rect::new(0, 0, 400, 400));
        for _ in 0..80 {
            menu.tick(16);
        }
        assert_eq!(menu.reveal.value(), 1.0, "the reveal must reach fully open");
        assert!(!menu.is_animating(), "a settled reveal owes no more frames");
        assert_eq!(menu.item_bands().len(), menu.items().len());
    }

    /// A configured cap windows the rows, and both the wheel and the hit test address the
    /// **visible** entry rather than its position in the whole list.
    #[test]
    fn a_configured_cap_windows_the_rows() {
        use crate::widget::menu_toolbar::effects::with_test_effects;

        let mut menu = Menu::new("File", Rect::new(0, 0, 200, 200));
        for index in 0..12 {
            menu.add_action(format!("Item {index}"));
        }
        let mut effects = crate::style::MenuEffects::legacy();
        effects.enabled = true;
        effects.max_visible_items = Some(5);

        with_test_effects(effects, || {
            menu.open_at(Point::new(0, 0), Rect::new(0, 0, 800, 600));
            assert_eq!(menu.visible_range(), 0..5);
            assert_eq!(menu.popup_height(), 5.0 * 22.0 + 4.0);

            // Wheel down scrolls one row at a time and stops at the last page.
            for _ in 0..20 {
                menu.handle_event(&Event::Wheel { delta: Point::new(0, -1), modifiers: 0 });
            }
            assert_eq!(menu.scroll_offset, 7, "12 rows - 5 visible = 7");
            assert_eq!(menu.visible_range(), 7..12);

            // The first painted row is entry 7, not entry 0.
            let first_row_y = menu.body_top() + 2 + 4;
            let hit = menu.item_index_at(Point::new(menu.geometry().x + 4, first_row_y));
            assert_eq!(hit, Some(7), "the hit test must follow the scrolled window");
        });
    }

    /// A row that owns children opens a real nested panel, and the child row commits.
    #[test]
    fn a_nested_panel_opens_and_its_child_row_commits() {
        let mut menu = Menu::new("Edit", Rect::new(0, 0, 180, 120));
        let branch = MenuEntry::new("Select")
            .with_submenu(vec![MenuEntry::new("All"), MenuEntry::new("None")]);
        menu.add_item(branch);
        menu.add_item(MenuEntry::new("Copy"));
        menu.open_at(Point::new(0, 0), Rect::new(0, 0, 600, 600));

        let row_y = menu.body_top() + 4;
        menu.handle_event(&Event::MousePress {
            pos: Point::new(20, row_y),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(menu.open_submenu_index(), Some(0), "the branch must open its children");
        let panel = menu.submenu_panel_rect().expect("an open panel has a rectangle");
        assert!(
            panel.x >= menu.geometry().x + menu.geometry().width as i32 - 8,
            "the panel hangs to the right of the parent: {panel:?}"
        );

        let chosen = Arc::new(AtomicUsize::new(0));
        let sink = Arc::clone(&chosen);
        menu.triggered.connect(move |_: Arc<String>| {
            sink.fetch_add(1, Ordering::SeqCst);
        });
        let child_y = panel.y + 2 + 4;
        menu.handle_event(&Event::MousePress {
            pos: Point::new(panel.x + 20, child_y),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(chosen.load(Ordering::SeqCst), 1, "a child row must commit");
        assert!(!menu.is_visible(), "committing a child dismisses the menu");
        assert_eq!(menu.open_submenu_index(), None, "hide clears the open panel");
    }

    /// Escape and Left unwind the nested panel before the menu itself.
    #[test]
    fn escape_closes_the_nested_panel_first() {
        let mut menu = Menu::new("Edit", Rect::new(0, 0, 180, 120));
        menu.add_item(
            MenuEntry::new("Select")
                .with_submenu(vec![MenuEntry::new("All"), MenuEntry::new("None")]),
        );
        menu.add_item(MenuEntry::new("Copy"));
        menu.open_at(Point::new(0, 0), Rect::new(0, 0, 600, 600));
        assert!(menu.open_submenu(0));
        assert!(menu.is_visible());

        menu.handle_event(&Event::KeyPress { key: 27, modifiers: 0 });
        assert_eq!(menu.open_submenu_index(), None, "Escape closes one level");
        assert!(menu.is_visible(), "the menu itself stays open after one Escape");
        menu.handle_event(&Event::KeyPress { key: 27, modifiers: 0 });
        assert!(!menu.is_visible(), "the second Escape dismisses the menu");
    }

    /// A menu whose branch chain is deeper than the control may open.
    ///
    /// Every level carries a branch named after the level below it plus a leaf named after its
    /// own level, and a second top-level branch exercises sibling replacement. The deepest
    /// branch ("Level 5") sits on level [`MAX_SUBMENU_DEPTH`], where opening is refused.
    fn menu_with_deep_chain() -> Menu {
        let level5 = MenuEntry::new("Level 5").with_submenu(vec![MenuEntry::new("Leaf 5")]);
        let level4 = MenuEntry::new("Level 4").with_submenu(vec![level5, MenuEntry::new("Leaf 4")]);
        let level3 = MenuEntry::new("Level 3").with_submenu(vec![level4, MenuEntry::new("Leaf 3")]);
        let level2 = MenuEntry::new("Level 2").with_submenu(vec![level3, MenuEntry::new("Leaf 2")]);
        let level1 = MenuEntry::new("Level 1").with_submenu(vec![level2, MenuEntry::new("Leaf 1")]);
        let mut menu = Menu::new("File", Rect::new(0, 0, 180, 120));
        menu.add_item(level1);
        menu.add_item(MenuEntry::new("Other").with_submenu(vec![MenuEntry::new("Other 1")]));
        menu.open_at(Point::new(0, 0), Rect::new(0, 0, 4000, 4000));
        menu
    }

    /// Presses row `index` of `level` through the control's pointer path.
    fn press_row(menu: &mut Menu, level: usize, index: usize) {
        let row = menu.row_rect_at(level, index).expect("the row exists");
        menu.handle_event(&Event::MousePress {
            pos: Point::new(row.x + 10, row.y + row.height as i32 / 2),
            button: 1,
            modifiers: 0,
        });
    }

    /// Four Rights open four panels, each to the right of the level that opened it.
    #[test]
    fn a_menu_opens_four_nested_panels() {
        let mut menu = menu_with_deep_chain();
        for _ in 0..MAX_SUBMENU_DEPTH {
            menu.handle_event(&Event::KeyPress { key: 39, modifiers: 0 });
        }

        assert_eq!(menu.submenu_depth(), MAX_SUBMENU_DEPTH);
        assert!(menu.is_visible(), "opening panels must never dismiss the menu");
        assert_eq!(menu.open_submenu_index(), Some(0), "the first step names the top branch");
        let mut previous = menu.geometry();
        for level in 1..=MAX_SUBMENU_DEPTH {
            let panel = menu.submenu_panel_rect_at(level).expect("each level has a panel");
            assert_eq!(
                panel.x,
                previous.x + previous.width as i32 - 6,
                "level {level} hangs off the level above it"
            );
            assert!(panel.width > 0 && panel.height > 0);
            previous = panel;
        }
    }

    /// The depth contract: a branch on the deepest level announces itself but opens nothing.
    #[test]
    fn a_menu_refuses_a_fifth_panel_but_announces_the_branch() {
        let mut menu = menu_with_deep_chain();
        let requested = Arc::new(Mutex::new(Vec::new()));
        let captured = requested.clone();
        menu.submenu_requested.connect(move |text| {
            captured.lock().unwrap().push(text.to_string());
        });
        let triggered = Arc::new(AtomicUsize::new(0));
        let captured_triggered = triggered.clone();
        menu.triggered.connect(move |_| {
            captured_triggered.fetch_add(1, Ordering::SeqCst);
        });
        for _ in 0..MAX_SUBMENU_DEPTH {
            menu.handle_event(&Event::KeyPress { key: 39, modifiers: 0 });
        }
        assert_eq!(menu.submenu_depth(), MAX_SUBMENU_DEPTH);

        // "Level 5" is a branch on the deepest allowed level.
        menu.handle_event(&Event::KeyPress { key: 39, modifiers: 0 });

        assert_eq!(menu.submenu_depth(), MAX_SUBMENU_DEPTH, "no fifth panel may open");
        assert!(menu.is_visible());
        assert_eq!(triggered.load(Ordering::SeqCst), 0, "a branch is not a committed choice");
        let after = requested.lock().unwrap().clone();
        assert_eq!(after.last().map(String::as_str), Some("Level 5"));
    }

    /// Escape unwinds one panel per press, deepest first, then the menu itself.
    #[test]
    fn escape_unwinds_every_nested_panel() {
        let mut menu = menu_with_deep_chain();
        for _ in 0..MAX_SUBMENU_DEPTH {
            menu.handle_event(&Event::KeyPress { key: 39, modifiers: 0 });
        }
        for expected in (0..MAX_SUBMENU_DEPTH).rev() {
            menu.handle_event(&Event::KeyPress { key: 27, modifiers: 0 });
            assert_eq!(menu.submenu_depth(), expected);
            assert!(menu.is_visible(), "Escape leaves the menu open until the last press");
        }
        menu.handle_event(&Event::KeyPress { key: 27, modifiers: 0 });
        assert!(!menu.is_visible());
    }

    /// A leaf in the deepest panel commits and dismisses through the pointer path.
    #[test]
    fn a_deep_leaf_commits_and_dismisses_the_menu() {
        let mut menu = menu_with_deep_chain();
        let chosen = Arc::new(Mutex::new(String::new()));
        let captured = chosen.clone();
        menu.triggered.connect(move |text| {
            *captured.lock().unwrap() = text.to_string();
        });
        for level in 0..MAX_SUBMENU_DEPTH {
            press_row(&mut menu, level, 0);
        }
        assert_eq!(menu.submenu_depth(), MAX_SUBMENU_DEPTH);

        press_row(&mut menu, MAX_SUBMENU_DEPTH, 1);

        assert_eq!(*chosen.lock().unwrap(), "Leaf 4");
        assert!(!menu.is_visible(), "a leaf selection dismisses the whole popup");
        assert_eq!(menu.submenu_depth(), 0);
    }

    /// Choosing a sibling branch replaces the deeper panels instead of stacking them.
    #[test]
    fn switching_a_parent_branch_discards_the_deeper_panels() {
        let mut menu = menu_with_deep_chain();
        press_row(&mut menu, 0, 0);
        press_row(&mut menu, 1, 0);
        assert_eq!(menu.submenu_depth(), 2);

        press_row(&mut menu, 0, 1);

        assert_eq!(menu.submenu_depth(), 1);
        let entries = menu.level_entries(1).expect("the sibling panel is open");
        assert_eq!(entries.first().map(MenuEntry::text), Some("Other 1"));
    }

    /// The hit rule must accept a press on any open panel, not just the popup rectangle.
    ///
    /// The panels hang to the right of the popup; the runtime asks `Widget::contains_point`
    /// before it delivers an event, so a control that only answered for its own rectangle
    /// made every child row unreachable at runtime even though the direct-event tests passed.
    #[test]
    fn contains_point_covers_every_open_panel() {
        let mut menu = menu_with_deep_chain();
        press_row(&mut menu, 0, 0);
        press_row(&mut menu, 1, 0);
        let panel = menu.submenu_panel_rect().expect("the deepest panel is open");
        let child = Point::new(panel.x + 2, panel.y + 2);
        assert!(
            !menu.geometry().contains_point(child),
            "the panel must lie outside the popup rectangle for this test to mean anything"
        );
        assert!(Widget::contains_point(&menu, child), "the runtime hit rule must accept the panel");
    }

    /// `&` markers parse to a mnemonic, strip from the drawn label, and answer Alt+letter.
    #[test]
    fn mnemonic_markers_parse_strip_and_activate() {
        use super::{menu_mnemonic, strip_menu_mnemonic};

        assert_eq!(menu_mnemonic("&File"), Some('F'));
        assert_eq!(menu_mnemonic("Save && Exit"), None, "a doubled marker is a literal ampersand");
        assert_eq!(strip_menu_mnemonic("&File"), "File");
        assert_eq!(strip_menu_mnemonic("Save && Exit"), "Save & Exit");
        assert_eq!(strip_menu_mnemonic("Plain"), "Plain");

        let mut menu = Menu::new("File", Rect::new(0, 0, 200, 120));
        menu.add_item(MenuEntry::new("&Open"));
        menu.add_item(MenuEntry::new("&Close"));
        menu.open_at(Point::new(0, 0), Rect::new(0, 0, 600, 600));

        let chosen = Arc::new(AtomicUsize::new(0));
        let sink = Arc::clone(&chosen);
        menu.triggered.connect(move |_: Arc<String>| {
            sink.fetch_add(1, Ordering::SeqCst);
        });
        // Alt is bit 2 in the event modifier convention.
        menu.handle_event(&Event::KeyPress { key: 79, modifiers: 0b0100 });
        assert_eq!(chosen.load(Ordering::SeqCst), 1, "Alt+O must run the &Open entry");
        assert!(!menu.is_visible());
    }

    /// A section caption is painted but never highlighted or chosen.
    #[test]
    fn section_captions_are_not_selectable() {
        let mut menu = Menu::new("File", Rect::new(0, 0, 200, 160));
        menu.add_item(MenuEntry::section("Recent"));
        menu.add_item(MenuEntry::new("One"));
        menu.open_at(Point::new(0, 0), Rect::new(0, 0, 600, 600));
        assert!(menu.items()[0].is_section());
        assert!(!menu.items()[0].is_selectable());

        menu.handle_event(&Event::KeyPress { key: 40, modifiers: 0 });
        assert_eq!(menu.hovered_index(), Some(1), "Down must skip the caption");
        assert_eq!(menu.mnemonic_index('r'), None, "a caption's letter is not an accelerator");
    }

    /// The tear-off strip occupies its own band and reports a press without dismissing.
    #[test]
    fn the_tearoff_strip_reports_a_press_and_keeps_the_menu_open() {
        let mut menu = Menu::new("File", Rect::new(0, 0, 200, 120));
        menu.add_item(MenuEntry::new("Open"));
        menu.open_at(Point::new(0, 0), Rect::new(0, 0, 600, 600));
        let without = menu.popup_height();
        menu.set_tearoff(true);
        assert_eq!(menu.popup_height(), without + 8.0, "the strip adds its own band");

        let torn = Arc::new(AtomicUsize::new(0));
        let sink = Arc::clone(&torn);
        menu.tearoff_requested.connect(move || {
            sink.fetch_add(1, Ordering::SeqCst);
        });
        menu.handle_event(&Event::MousePress {
            pos: Point::new(40, menu.body_top() + 4),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(torn.load(Ordering::SeqCst), 1);
        assert!(menu.is_visible(), "tearing off must not dismiss the source menu");
    }

    /// Auto flips a popup up when the space below the anchor cannot hold it.
    #[test]
    fn the_popup_direction_anchors_up_when_the_bottom_is_crowded() {
        let viewport = Rect::new(0, 0, 220, 160);
        let mut auto = context_menu();
        auto.open_at(Point::new(10, 150), viewport);
        let total = auto.geometry().height as i32;
        assert!(
            auto.geometry().y + total <= 150,
            "an Auto menu must end at or above the anchor: {}",
            auto.geometry().y
        );
        assert!(auto.geometry().y >= 0, "and stay inside the viewport");

        // Down keeps the historical top-left anchor when there is room.
        let wide = Rect::new(0, 0, 800, 800);
        let mut down = context_menu();
        down.set_popup_direction(PopupDirection::Down);
        down.open_at(Point::new(100, 200), wide);
        assert_eq!(down.geometry().y, 200);

        let mut up = context_menu();
        up.set_popup_direction(PopupDirection::Up);
        up.open_at(Point::new(100, 200), wide);
        assert_eq!(up.geometry().y + up.geometry().height as i32, 200);
    }

    /// The host can place the highlight on a row by index.
    #[test]
    fn set_active_action_highlights_only_a_selectable_row() {
        let mut menu = context_menu();
        menu.open_at(Point::new(0, 0), Rect::new(0, 0, 400, 400));
        assert!(menu.set_active_action(1));
        assert_eq!(menu.hovered_index(), Some(1));

        // Index 2 is the separator: it is not a destination.
        assert!(!menu.set_active_action(2));
        assert_eq!(menu.hovered_index(), Some(1), "a refused write must not move the highlight");
        assert!(!menu.set_active_action(99));
    }

    /// Items can be inserted, removed and looked up by their action id.
    #[test]
    fn items_can_be_inserted_and_removed_by_position_and_action() {
        let mut menu = Menu::new("File", Rect::new(0, 0, 200, 120));
        menu.add_action("A");
        menu.add_action("C");
        let at = menu.insert_item(1, MenuEntry::new("B"));
        assert_eq!(at, 1);
        assert_eq!(menu.items().iter().map(MenuEntry::text).collect::<Vec<_>>(), ["A", "B", "C"]);

        let removed = menu.remove_item(1).expect("an in-range index returns the row");
        assert_eq!(removed.text(), "B");
        assert!(menu.remove_item(99).is_none());

        menu.insert_separator(1);
        assert!(menu.items()[1].is_separator());

        let mut action = Action::new("Save", Rect::new(0, 0, 80, 22));
        action.set_command_id("save");
        menu.add_action_item(&action);
        assert!(menu.remove_action("save"), "the linked row is found by command id");
        assert!(!menu.remove_action("save"), "and removed only once");
    }

    /// An action's metadata reaches the row it creates.
    #[test]
    fn add_action_item_mirrors_the_action() {
        let mut action = Action::new("Save", Rect::new(0, 0, 80, 22));
        action.set_command_id("save");
        action.set_checkable(true);
        action.set_checked(true);
        action.set_status_tip("Write the document");
        let id = action.base().id();

        let mut menu = Menu::new("File", Rect::new(0, 0, 200, 120));
        let index = menu.add_action_item(&action);
        let row = &menu.items()[index];
        assert_eq!(row.text(), "Save");
        assert_eq!(row.action_id(), Some("save"));
        assert_eq!(row.action(), Some(id), "the live widget identity is recorded");
        assert_eq!(row.tooltip(), "Write the document");
        assert!(row.is_checkable() && row.is_checked());
    }

    /// A column count lays rows out as a row-major grid and navigation follows it.
    #[test]
    fn a_column_count_makes_a_grid_that_the_pointer_and_keys_follow() {
        let mut menu = Menu::new("File", Rect::new(0, 0, 400, 200));
        for index in 0..6 {
            menu.add_action(format!("Item {index}"));
        }
        menu.set_column_count(2);
        menu.open_at(Point::new(0, 0), Rect::new(0, 0, 800, 800));
        assert_eq!(menu.columns(), 2);

        let bands = menu.item_bands();
        assert_eq!(bands[1].y, bands[0].y, "the second entry shares the first row");
        assert!(
            bands[1].x >= bands[0].x + bands[0].width as i32,
            "and sits in the next column: {bands:?}"
        );
        assert_eq!(bands[2].x, bands[0].x, "entry two starts the second row");
        assert_eq!(bands[2].y, bands[0].y + Menu::item_height() as i32);

        assert!(menu.set_active_action(0));
        menu.handle_event(&Event::KeyPress { key: 40, modifiers: 0 });
        assert_eq!(menu.hovered_index(), Some(2), "Down moves one grid row");
        menu.handle_event(&Event::KeyPress { key: 39, modifiers: 0 });
        assert_eq!(menu.hovered_index(), Some(3), "Right moves one column");

        let row = bands[3];
        let pos = Point::new(menu.geometry().x + row.x + 2, menu.body_top() + row.y + 2);
        assert_eq!(menu.item_index_at(pos), Some(3), "the hit test addresses the column");
    }
}
