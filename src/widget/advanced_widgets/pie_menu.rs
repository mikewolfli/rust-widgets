// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! PieMenu (radial menu) widget.
//!
//! A circular popup menu that displays items as radial slices.
//! Users hover to highlight and click to select an item.
//!
//! A slice is icon-led: a typed icon (`set_item_icon_source`) or a text stand-in
//! (`icon_text`) is drawn instead of the label when one is present. The hovered item's
//! name reads out in the centre hub — the label's legible moment — and the idle hub keeps
//! its centre dot.

use std::f32::consts::TAU;

use crate::core::{Color, HorizontalAlignment, Point, Rect, Size};
use crate::event::key_codes;
use crate::event::{Event, EventHandler};
use crate::render::{RenderContext, TEXT_FIT_MARGIN};
use crate::signal::{GenericSignal, Signal1};
use crate::style::{MotionSlot, PropertyDriver};
use crate::widget::capability::coercion::{expect_f32, expect_usize};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{dimensions, effective_font};
use crate::widget::{BaseWidget, Draw, IconItemError, IconSource, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// The record form of the `items` property: one row per pie item.
///
/// # Why the row is read-only
///
/// `PieMenuItem` is crate-constructed and carries slice angles the menu recomputes, plus an id the
/// menu must keep unique. A writable row would have to invent angles and could collide two items on
/// one id — the silent-loss shape blue27 §19.6 forbids. The rows publish what a document can hold
/// faithfully (identity, label, text icon, typed icon, enabled) and a write is refused with the
/// standard read-only error; behaviour changes through the control's own API
/// (`add_item_with_id` / `set_item_icon_source`).
pub const PIE_MENU_ITEM_SCHEMA: crate::widget::capability::RecordSchema =
    crate::widget::capability::RecordSchema {
        fields: &[
            crate::widget::capability::RecordField::required(
                "id",
                crate::widget::capability::PropertyValueKind::UInt,
            ),
            crate::widget::capability::RecordField::required(
                "text",
                crate::widget::capability::PropertyValueKind::String,
            ),
            crate::widget::capability::RecordField::optional(
                "icon",
                crate::widget::capability::PropertyValueKind::String,
            ),
            crate::widget::capability::RecordField::optional(
                "icon_source",
                crate::widget::capability::PropertyValueKind::IconSpec,
            ),
            crate::widget::capability::RecordField::optional(
                "enabled",
                crate::widget::capability::PropertyValueKind::Bool,
            ),
            // The shortcut label painted near the wedge's outer edge; absent means none.
            crate::widget::capability::RecordField::optional(
                "shortcut",
                crate::widget::capability::PropertyValueKind::String,
            ),
            // Whether the slot is a gap rather than a choice.
            crate::widget::capability::RecordField::optional(
                "separator",
                crate::widget::capability::PropertyValueKind::Bool,
            ),
        ],
        id_field: "id",
        // `id` is a `UInt`, and the list's identity rules read text only (`RecordValue::text`
        // answers "" for a non-text field), so an enforced-unique rule here would report every
        // row as an empty id. Uniqueness is enforced by the control's own `add_item_with_id`,
        // which refuses a duplicate with `DuplicateItemId`; the write path maps that refusal to a
        // constraint violation.
        ids_unique: false,
        ids_required: false,
    };

/// One row of [`PIE_MENU_ITEM_SCHEMA`]; refuses a source with no document form.
fn pie_menu_item_record(
    item: &PieMenuItem,
) -> Result<crate::widget::capability::RecordValue, CapabilityAccessError> {
    let mut record = crate::widget::capability::RecordValue::new()
        .with("id", CapabilityValue::UInt(item.id()))
        .with("text", CapabilityValue::String(item.text().to_string()))
        .with("icon", CapabilityValue::String(item.icon_text().to_string()))
        .with("enabled", CapabilityValue::Bool(item.is_enabled()));
    match item.icon_source() {
        // No typed override: the field is omitted, which is how a document says "no typed icon"
        // (the item's text icon travels in its own field).
        None => {}
        Some(IconSource::Builtin(name)) => {
            record = record.with(
                "icon_source",
                CapabilityValue::IconSpec(crate::widget::icon_source::IconSpec::Builtin(name)),
            );
        }
        // A borrowed static geometry has no document form, so the read refuses rather than
        // reporting the row as icon-less — the rule the control-level `icon_source` owners follow.
        Some(IconSource::StaticGeometry(_)) => {
            return Err(CapabilityAccessError::IconSourceNotPersistable)
        }
    }
    if let Some(shortcut) = item.shortcut() {
        record = record.with("shortcut", CapabilityValue::String(shortcut.to_string()));
    }
    if item.is_separator() {
        record = record.with("separator", CapabilityValue::Bool(true));
    }
    Ok(record)
}

/// Rebuilds one [`PieMenuItem`] from a row of [`PIE_MENU_ITEM_SCHEMA`].
///
/// Child rings are **not** part of the schema: a record field cannot carry a nested record list
/// (`value_matches_kind` refuses it), so a write rebuilds the flat top-level ring and the rows it
/// declares carry no children — the same "the document is authoritative" rule a chip's omitted
/// icon follows. A caller that needs nested rings keeps using the control's own API.
fn pie_menu_item_from_record(
    record: &crate::widget::capability::RecordValue,
) -> Result<PieMenuItem, CapabilityAccessError> {
    let id = match record.get("id") {
        Some(CapabilityValue::UInt(id)) => *id,
        // `Int` is accepted as well: a JSON reader may hand an integral number either way.
        Some(CapabilityValue::Int(id)) if *id >= 0 => *id as u64,
        _ => return Err(CapabilityAccessError::TypeMismatch),
    };
    let text = match record.get("text") {
        Some(CapabilityValue::String(text)) => text.clone(),
        _ => return Err(CapabilityAccessError::TypeMismatch),
    };
    let icon = match record.get("icon") {
        None | Some(CapabilityValue::Null) => String::new(),
        Some(CapabilityValue::String(icon)) => icon.clone(),
        Some(_) => return Err(CapabilityAccessError::TypeMismatch),
    };
    let enabled = match record.get("enabled") {
        None | Some(CapabilityValue::Null) => true,
        Some(CapabilityValue::Bool(flag)) => *flag,
        Some(_) => return Err(CapabilityAccessError::TypeMismatch),
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
    let shortcut = match record.get("shortcut") {
        None | Some(CapabilityValue::Null) => None,
        Some(CapabilityValue::String(shortcut)) => Some(shortcut.clone()),
        Some(_) => return Err(CapabilityAccessError::TypeMismatch),
    };
    let separator = match record.get("separator") {
        None | Some(CapabilityValue::Null) => false,
        Some(CapabilityValue::Bool(flag)) => *flag,
        Some(_) => return Err(CapabilityAccessError::TypeMismatch),
    };

    let mut item = PieMenuItem::new(text);
    item.id = id;
    item.set_icon_text(icon);
    item.set_icon_source(icon_source);
    item.set_enabled(enabled);
    item.set_shortcut(shortcut);
    item.set_separator(separator);
    Ok(item)
}

/// A single item in a `PieMenu`.
#[derive(Debug, Clone)]
pub struct PieMenuItem {
    /// Stable identity within the menu, unique among its items.
    ///
    /// The menu assigns one to every item it is asked to append without an explicit id, and a host
    /// may pin one instead (a document's identity for the row). It is what a typed icon is addressed
    /// by, so an assignment cannot land on a different item after an insertion or removal — the
    /// failure a position-keyed side table allows (blue27 §19.6).
    id: u64,
    text: String,
    icon_text: String,
    /// Typed icon owned by the item, so structural edits carry it along instead of shifting it.
    icon_source: Option<IconSource>,
    enabled: bool,
    angle_start: f32,
    angle_end: f32,
    /// Optional shortcut label painted near the wedge's outer edge.
    shortcut: Option<String>,
    /// Whether the slot is a gap rather than a choice.
    separator: bool,
    /// Child items, shown as a second ring when this slot is activated.
    children: Vec<PieMenuItem>,
}

impl PieMenuItem {
    /// Creates an enabled item with the given label, no icon text, and its
    /// angles unset (`0.0` for both).
    ///
    /// The angles are placeholders until the item is inserted into a
    /// [`PieMenu`], which recomputes them to divide the circle evenly among all
    /// items.
    ///
    /// The item carries no identity yet — the menu assigns one when it appends the item, so
    /// [`PieMenuItem::id`] is only meaningful afterwards. Use [`PieMenu::add_item_with_id`] when
    /// the id has to be the caller's own (a document's identity for the row).
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            id: 0,
            text: text.into(),
            icon_text: String::new(),
            icon_source: None,
            enabled: true,
            angle_start: 0.0,
            angle_end: 0.0,
            shortcut: None,
            separator: false,
            children: Vec::new(),
        }
    }

    /// The item's stable identity within its menu.
    pub fn id(&self) -> u64 {
        self.id
    }

    /// The typed icon assigned to this item, if any.
    pub fn icon_source(&self) -> Option<IconSource> {
        self.icon_source
    }

    /// Assigns the item's typed icon.
    ///
    /// [`PieMenu::set_item_icon_source`] is the collection-level entry point and refuses an
    /// unknown id; this is the same assignment without the lookup.
    pub fn set_icon_source(&mut self, source: Option<IconSource>) {
        self.icon_source = source;
    }

    /// Returns the item's label.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Replaces the item's label. The slice angle is not affected, and no
    /// redraw is requested.
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
    }

    /// Returns the icon text, or an empty string when the item has no icon.
    ///
    /// This is a text stand-in for an icon, not image data.
    pub fn icon_text(&self) -> &str {
        &self.icon_text
    }

    /// Replaces the icon text. An empty string means "no icon".
    pub fn set_icon_text(&mut self, icon: impl Into<String>) {
        self.icon_text = icon.into();
    }

    /// Returns whether the item can be clicked. Defaults to `true`.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Enables or disables the item. A disabled item is still drawn but is
    /// skipped by hit testing, so hovering it clears the hover highlight rather
    /// than selecting it.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Returns the slice's start angle, in radians measured clockwise from the
    /// positive x axis (see the module drawing code), with `0.0` at the right
    /// of the menu centre.
    ///
    /// Managed by [`PieMenu`]; setting it directly is only meaningful for a
    /// standalone item.
    pub fn angle_start(&self) -> f32 {
        self.angle_start
    }

    /// Sets the slice's start angle in radians. Not validated, and the menu
    /// overwrites it whenever the item list changes.
    pub fn set_angle_start(&mut self, angle: f32) {
        self.angle_start = angle;
    }

    /// Returns the slice's exclusive end angle, in radians. The slice spans
    /// `angle_start .. angle_end`.
    pub fn angle_end(&self) -> f32 {
        self.angle_end
    }

    /// Sets the slice's exclusive end angle in radians. Not validated.
    pub fn set_angle_end(&mut self, angle: f32) {
        self.angle_end = angle;
    }

    /// The shortcut label painted near this wedge's outer edge, if any.
    pub fn shortcut(&self) -> Option<&str> {
        self.shortcut.as_deref()
    }

    /// Sets the shortcut label.
    pub fn set_shortcut(&mut self, shortcut: Option<String>) {
        self.shortcut = shortcut;
    }

    /// Sets the shortcut label and returns `self`.
    pub fn with_shortcut(mut self, shortcut: impl Into<String>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }

    /// Whether this slot is a gap rather than a choice.
    pub fn is_separator(&self) -> bool {
        self.separator
    }

    /// Marks the slot as a separator: it paints no wedge and cannot be hovered.
    pub fn set_separator(&mut self, separator: bool) {
        self.separator = separator;
    }

    /// The child slots shown as the second ring, empty for a leaf.
    pub fn children(&self) -> &[PieMenuItem] {
        &self.children
    }

    /// Attaches child slots, making this item a branch.
    pub fn set_children(&mut self, children: Vec<PieMenuItem>) {
        self.children = children;
    }

    /// Attaches child slots and returns `self`.
    pub fn with_children(mut self, children: Vec<PieMenuItem>) -> Self {
        self.children = children;
        self
    }
}

/// PieMenu (radial/circular menu) widget.
///
/// Displays items arranged radially around a center point. The menu
/// appears as a donut-like ring with labelled slices. Supports hover
/// highlighting, click selection, and keyboard dismissal.
pub struct PieMenu {
    base: BaseWidget,
    items: Vec<PieMenuItem>,
    /// The next id an item appended without one receives.
    ///
    /// Ids are unique within the menu: appending without an id takes this one, and pinning a higher
    /// id pushes this past it, so a host-chosen id can never be handed out twice.
    next_id: u64,
    radius: f32,
    inner_radius: f32,
    hovered_index: Option<usize>,
    current_index: usize,
    /// The branch path whose child ring is showing, outermost first.
    ///
    /// Each entry is the slot index **within the ring above it**, so the shown ring is
    /// `items[path[0]].children[path[1]].children[path[2]]…` and an empty path is the top level.
    /// This replaces the single `Option<usize>` that could address exactly one level: a branch
    /// inside a branch was drawn (its children were distributed on open) but unreachable, because
    /// `activate` refused to open a ring while one was already open. Escape already unwound one
    /// ring at a time in the plan, so the fix is a stack and the existing pop becomes the whole
    /// behaviour.
    submenu_path: Vec<usize>,
    center: Point,
    /// How far the menu has opened, `0.0` at rest and `1.0` fully shown.
    ///
    /// # Why this is a driver and not a bare `f32`
    ///
    /// It used to be a plain number that started at `1.0` and that **nothing read** — not
    /// `draw`, not any accessor beyond its own getter. So the field was dead state, and the
    /// one thing it asserted was wrong: it began at the *target* end, which is the shape
    /// BLUE24 §2.4 gate B exists to catch ("a control constructed already finished").
    ///
    /// As a [`PropertyDriver`] the value is owned by the same type every other animated
    /// control uses, starts at the resting end, is driven by [`PieMenu::tick`], and is read
    /// by `draw` to scale the ring out as the menu opens — so the declaration and the picture
    /// are the same fact.
    animation_progress: PropertyDriver,
    /// The caller's hover highlight, or `None` while the highlight follows the active
    /// theme's `primary` token.
    ///
    /// The field used to be a plain `Color` seeded with `rgb(0, 120, 215)` — a fixed blue
    /// painted identically on every appearance, the same theme-blindness the sibling
    /// `menu` popup shed when its hovered row moved onto the theme's primary. An unset
    /// colour resolves through [`Self::theme_hover_color`] (the fixed literal only when
    /// no theme is active) and [`PieMenu::set_hover_color`] still wins when the caller
    /// states one.
    hover_color: Option<Color>,
    /// The caller's label ink, or `None` while labels follow the resolved theme ink.
    ///
    /// It used to be a fixed `rgb(30, 30, 30)` — a near-black that only reads on light
    /// wedges. An unset colour is drawn in the same `ink` `draw` already resolves for the
    /// ring's chrome, so labels and chrome move with the theme together.
    text_color: Option<Color>,
    /// Emitted with the index whose selection was applied. Fires from user
    /// clicks and from [`PieMenu::set_current_index`] — so programmatic
    /// selection is indistinguishable from a click, and a slot that reacts by
    /// calling `set_current_index` again will recurse. A
    /// [`PieMenu::set_current_index`] call with an out-of-range index emits
    /// nothing.
    pub triggered: Signal1<usize>,
    /// Emitted with the selected item's label, alongside `triggered`. Empty
    /// labels produce an empty payload; duplicate labels are indistinguishable.
    pub triggered_text: Signal1<String>,
    /// Emitted with the selected item's **id**, alongside `triggered`.
    ///
    /// Items are identified by id everywhere else in the public surface ([`PieMenu::add_item`]
    /// returns one, [`PieMenu::set_item_icon_source`] addresses by one), while `triggered` carries
    /// an index that moves when an item is inserted or removed. A host that stores ids therefore
    /// had no signal it could match against them without re-deriving the mapping.
    pub triggered_id: Signal1<u64>,
    /// Emitted by [`PieMenu::show_at`], just before the menu becomes visible.
    pub about_to_show: GenericSignal,
    /// Emitted by [`PieMenu::hide`], after the menu is hidden and the hover
    /// highlight cleared.
    pub about_to_hide: GenericSignal,
    /// Reused point/contour buffers for the ring and slice paths (K-P07).
    ///
    /// `draw` rebuilds every contour from scratch each frame, and each builder used to allocate
    /// its own `Vec`s — a ring plus one per wedge, so an eight-item menu allocated ten buffers a
    /// frame for point lists whose length depends only on the item count. These buffers keep their
    /// capacity across frames; `clear()` at the start of each contour is the only reuse protocol.
    paint_scratch: crate::compat::RefCell<PieMenuScratch>,
    /// The wedges' contours, rebuilt only when the geometry or the item angles change.
    ///
    /// A wedge is a pure function of the centre, the two radii and its two angles, so a static
    /// frame used to rebuild every contour — one sample per two pixels of arc, per wedge — to
    /// reproduce byte-for-byte the same points (ADV-006). Hover changes only the fill colour,
    /// which is read after the contour, so it does not invalidate the cache.
    slice_paths: crate::compat::RefCell<SlicePathCache>,
}

/// The scratch buffers a [`PieMenu`] reuses while building its paths.
#[derive(Default)]
struct PieMenuScratch {
    /// The points of the contour being filled.
    points: Vec<Point>,
    /// The `(start, end)` ranges of the contours inside [`Self::points`].
    contours: Vec<(usize, usize)>,
}

/// The inputs that decide a ring's wedge geometry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SlicePathKey {
    center_x: i32,
    center_y: i32,
    outer_bits: u32,
    inner_bits: u32,
    /// FNV-1a over every active item's start/end angle bits, so an angle edit invalidates.
    angles_hash: u64,
    /// The number of slots the cached list is indexed by, including separators.
    item_count: usize,
}

impl SlicePathKey {
    /// Folds the draw inputs into one comparable value.
    fn of(center: Point, outer: f32, inner: f32, items: &[PieMenuItem]) -> Self {
        let mut angles_hash: u64 = 0xcbf2_9ce4_8422_2325;
        for item in items {
            // The separator flag decides whether the slot paints at all, so it is part of the
            // geometry key even though a separator has no meaningful angles.
            let mut mix = |byte: u8| {
                angles_hash ^= u64::from(byte);
                angles_hash = angles_hash.wrapping_mul(0x0000_0100_0000_01b3);
            };
            mix(u8::from(item.is_separator()));
            for bits in [item.angle_start().to_bits(), item.angle_end().to_bits()] {
                for byte in bits.to_le_bytes() {
                    mix(byte);
                }
            }
        }
        angles_hash ^= items.len() as u64;
        Self {
            center_x: center.x,
            center_y: center.y,
            outer_bits: outer.to_bits(),
            inner_bits: inner.to_bits(),
            angles_hash,
            item_count: items.len(),
        }
    }
}

/// One wedge's contour, in the form [`RenderContext::fill_path_non_zero`] takes.
#[derive(Default)]
struct SlicePath {
    /// The contour's points; empty when the slot paints nothing (a separator).
    points: Vec<Point>,
    /// The `(start, end)` range of the contour inside [`Self::points`].
    contour: (usize, usize),
}

/// The per-slot wedge contours and the inputs they were built for.
#[derive(Default)]
struct SlicePathCache {
    /// `None` before the first build, so an empty key can never be mistaken for a hit.
    key: Option<SlicePathKey>,
    /// One entry per slot in `active_items()`, including separators (empty paths).
    slices: Vec<SlicePath>,
}

impl PieMenu {
    /// The smallest outer radius [`PieMenu::new`] and [`PieMenu::set_radius`] accept.
    ///
    /// Exposed because the two are the *only* writers of `radius`, and both floor at this
    /// value, which is what makes the `2.0` floor of [`PieMenu::set_inner_radius`] and the
    /// `0.95 * radius` ceiling mutually satisfiable. `new` used to accept any value — including
    /// zero, which produced a zero-sized geometry no item can be hit-tested against.
    pub const MIN_RADIUS: f32 = 10.0;

    /// Creates a new `PieMenu` centered at `center` with the given outer `radius`.
    ///
    /// `radius` is floored at [`PieMenu::MIN_RADIUS`], the same floor [`PieMenu::set_radius`]
    /// applies, so a menu never starts outside the range its own setters maintain. The inner
    /// radius starts at 35% of the **effective** radius, keeping it inside the
    /// `2.0 ..= 0.95 * radius` band [`PieMenu::set_inner_radius`] documents.
    pub fn new(center: Point, radius: f32) -> Self {
        let radius =
            if radius.is_finite() { radius.max(Self::MIN_RADIUS) } else { Self::MIN_RADIUS };
        let size = (radius * 2.0) as u32;
        let geometry = Rect::new(center.x - radius as i32, center.y - radius as i32, size, size);
        let inner_radius = radius * 0.35;
        Self {
            base: BaseWidget::new(WidgetKind::PieMenu, geometry, "PieMenu"),
            items: Vec::new(),
            next_id: 1,
            radius,
            inner_radius,
            hovered_index: None,
            current_index: 0,
            submenu_path: Vec::new(),
            center,
            // At rest, so a freshly built menu opens rather than appearing already open. The
            // ring unfurling is a larger movement than a pointer reaction, so it is priced by
            // the theme's `slow` token.
            animation_progress: PropertyDriver::at(0.0, MotionSlot::Slow),
            hover_color: None,
            text_color: None,
            triggered: Signal1::new(),
            triggered_text: Signal1::new(),
            triggered_id: Signal1::new(),
            about_to_show: GenericSignal::new(),
            about_to_hide: GenericSignal::new(),
            paint_scratch: crate::compat::RefCell::new(PieMenuScratch::default()),
            slice_paths: crate::compat::RefCell::new(SlicePathCache::default()),
        }
    }

    /// Returns the selected index, or `None` when the menu has no items.
    pub fn selected_index(&self) -> Option<usize> {
        self.items.get(self.current_index).map(|_| self.current_index)
    }

    /// Returns the current (last selected) index.
    #[deprecated(note = "use `selected_index` (Option) — WDGT-13")]
    pub fn current_index(&self) -> usize {
        self.current_index
    }

    /// Sets the current index with bounds checking.
    ///
    /// An out-of-range index is silently ignored. On success this emits
    /// `triggered` and `triggered_text` and requests a redraw; the current
    /// index is **not** updated for an item that is disabled.
    pub fn set_current_index(&mut self, idx: usize) {
        if idx < self.items.len() {
            // Mirror the click/hit-test paths: a disabled item is not selectable,
            // so the index is left untouched and no signal fires.
            if !self.items[idx].is_enabled() {
                return;
            }
            self.current_index = idx;
            self.triggered.emit(idx);
            if let Some(text) = self.items.get(idx).map(|item| item.text().to_string()) {
                self.triggered_text.emit(text);
            }
            if let Some(id) = self.items.get(idx).map(PieMenuItem::id) {
                self.triggered_id.emit(id);
            }
            self.base.request_redraw();
        }
    }

    /// Adds a menu item and returns the id the menu assigned it.
    ///
    /// The id is what [`PieMenu::set_item_icon_source`] addresses the item by, which is why it is
    /// returned rather than the index: an index only names the item until the next insertion.
    pub fn add_item(&mut self, text: impl Into<String>) -> u64 {
        self.add_item_with_icon(text, "")
    }

    /// Adds an item that already carries presentation state and returns the id the
    /// menu assigned it.
    ///
    /// This is the entry point for a shortcut hint, a separator slot, or a branch with
    /// children — state [`PieMenu::add_item`] cannot express.
    pub fn add_existing_item(&mut self, mut item: PieMenuItem) -> u64 {
        let id = if item.id == 0 {
            self.take_next_id()
        } else {
            self.next_id = self.next_id.max(item.id.saturating_add(1));
            item.id
        };
        item.id = id;
        self.items.push(item);
        self.recalculate_angles();
        self.base.request_redraw();
        id
    }

    /// Adds an item with a text icon and returns the id the menu assigned it.
    ///
    /// Angles are recomputed so all items share the circle equally, which means
    /// adding an item moves every existing slice.
    pub fn add_item_with_icon(&mut self, text: impl Into<String>, icon: impl Into<String>) -> u64 {
        let mut item = PieMenuItem::new(text);
        item.set_icon_text(icon);
        let id = self.take_next_id();
        item.id = id;
        self.items.push(item);
        self.recalculate_angles();
        id
    }

    /// Adds an item whose id the caller chose — the identity a document carries.
    ///
    /// The id must not already exist. Ids the menu assigns from now on skip past a pinned one, so
    /// the two sources cannot collide.
    pub fn add_item_with_id(
        &mut self,
        id: u64,
        text: impl Into<String>,
    ) -> Result<u64, IconItemError> {
        if self.items.iter().any(|item| item.id == id) {
            // The numeric-identity variant: `DuplicateItemId` carries a text id (a chip's), and
            // widening one payload to cover both kinds would make callers convert an id they
            // already hold in the right type.
            return Err(IconItemError::AmbiguousItemId { id });
        }
        let mut item = PieMenuItem::new(text);
        item.id = id;
        // Auto-assigned ids skip past a pinned one, so the two sources cannot collide.
        self.next_id = self.next_id.max(id.saturating_add(1));
        self.items.push(item);
        self.recalculate_angles();
        Ok(id)
    }

    /// The next id to hand out, and the counter moved past it.
    fn take_next_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        id
    }

    /// Assigns a typed icon source to the item with this id.
    ///
    /// Addressing by id rather than by position is what keeps an assignment attached to the item it
    /// was made for: inserting or removing another item may move indices, but it cannot change
    /// which item an id names (blue27 §19.6).
    pub fn set_item_icon_source(
        &mut self,
        id: u64,
        source: Option<IconSource>,
    ) -> Result<(), IconItemError> {
        let Some(item) = self.items.iter_mut().find(|item| item.id == id) else {
            return Err(IconItemError::ItemNotFound { id });
        };
        item.set_icon_source(source);
        self.base.request_redraw();
        Ok(())
    }

    /// The typed icon assigned to the item with this id.
    pub fn item_icon_source(&self, id: u64) -> Option<IconSource> {
        self.items.iter().find(|item| item.id == id).and_then(PieMenuItem::icon_source)
    }

    /// The id of the item at `index`, or `None` when the index is out of range.
    pub fn item_id(&self, index: usize) -> Option<u64> {
        self.items.get(index).map(PieMenuItem::id)
    }

    /// Inserts an item at `index`, or appends when `index` is past the end.
    ///
    /// The new item has no icon text. Angles are recomputed for every item, and
    /// [`PieMenu::current_index`] is not adjusted, so it can end up naming a
    /// different item than before.
    pub fn insert_item(&mut self, index: usize, text: impl Into<String>) {
        let idx = index.min(self.items.len());
        let mut item = PieMenuItem::new(text);
        item.id = self.take_next_id();
        self.items.insert(idx, item);
        self.recalculate_angles();
    }

    /// Removes the item at `index`; out-of-range indices are ignored.
    ///
    /// Angles are recomputed for the remaining items. [`PieMenu::current_index`]
    /// and [`PieMenu::hovered_index`] are not adjusted, so they can be left
    /// pointing past the end of the list.
    pub fn remove_item(&mut self, index: usize) {
        if index < self.items.len() {
            self.items.remove(index);
            self.recalculate_angles();
        }
    }

    /// Removes all menu items and clears the hover highlight.
    ///
    /// [`PieMenu::current_index`] is left as-is even though it now names no
    /// item.
    pub fn clear(&mut self) {
        self.items.clear();
        self.hovered_index = None;
    }

    /// Returns the number of items in the menu.
    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    /// Returns a slice of all menu items.
    /// The items, for inspection.
    pub fn items(&self) -> &[PieMenuItem] {
        &self.items
    }

    /// The items, mutably; angle fields are recomputed by the menu, so a caller that
    /// changes bounds here should re-add the item or use the setters.
    pub fn items_mut(&mut self) -> &mut [PieMenuItem] {
        &mut self.items
    }

    /// Enables or disables the item at `index`, ignoring out-of-range indices.
    /// Disabling does not clear an existing hover highlight or redraw.
    pub fn set_item_enabled(&mut self, index: usize, enabled: bool) {
        if let Some(item) = self.items.get_mut(index) {
            item.set_enabled(enabled);
        }
    }

    /// Returns the outer radius of the menu.
    pub fn radius(&self) -> f32 {
        self.radius
    }

    /// Sets the outer radius of the menu.
    ///
    /// Values below `10.0` are raised to `10.0` ([`PieMenu::MIN_RADIUS`]); a non-finite value
    /// falls back to that floor. The inner radius is pulled down if it would otherwise reach
    /// past 90% of the new outer radius, and the widget geometry is recomputed to the
    /// enclosing square.
    /// Sets the outer radius, clamping it to a finite value no smaller than
    /// [`Self::MIN_RADIUS`] and shrinking the inner radius to stay inside it.
    ///
    /// Repaints: `Self::update_geometry` calls `BaseWidget::set_geometry`, which
    /// deliberately stores the rectangle **without** requesting a redraw (see its own docs),
    /// so the caller here owes one.
    pub fn set_radius(&mut self, radius: f32) {
        let radius =
            if radius.is_finite() { radius.max(Self::MIN_RADIUS) } else { Self::MIN_RADIUS };
        if self.radius == radius {
            return;
        }
        self.radius = radius;
        self.inner_radius = self.inner_radius.min(self.radius * 0.9);
        self.update_geometry();
        self.base.request_redraw();
    }

    /// Returns the inner (donut hole) radius.
    pub fn inner_radius(&self) -> f32 {
        self.inner_radius
    }

    /// Sets the inner (donut hole) radius.
    ///
    /// Clamped into `2.0 ..= 0.95 * radius`; the widget geometry is recomputed.
    ///
    /// # Why the clamp order matters
    ///
    /// This was `inner_radius.max(2.0).min(self.radius * 0.95)`: raising the floor first and
    /// only then applying the ceiling means the ceiling wins, so the documented lower bound is
    /// not actually guaranteed when `0.95 * radius < 2.0`. That required `radius < ~2.11`,
    /// which [`PieMenu::set_radius`] could not produce (it floors at 10.0) but
    /// [`PieMenu::new`] **could**, because it accepted any radius: `new(center, 2.0)` followed
    /// by `set_inner_radius(1.0)` stored `1.9`, below the documented floor. `min` then `max`
    /// makes the band's own definition authoritative, and with both writers now flooring the
    /// radius the two bounds cannot cross.
    ///
    /// A non-finite input is refused (the previous value is kept) rather than propagated:
    /// `f32::max`/`f32::min` return the non-NaN operand, so a NaN input silently produced
    /// whatever the other operand was, which is not a value the caller asked for either.
    /// Sets the inner (donut hole) radius, clamped inside the outer radius and to at least 2 px.
    ///
    /// Repaints: `update_geometry` only stores the rectangle (see
    /// [`crate::widget::BaseWidget::set_geometry`]), so a caller must ask for the frame.
    pub fn set_inner_radius(&mut self, inner_radius: f32) {
        if !inner_radius.is_finite() {
            return;
        }
        let inner_radius = inner_radius.min(self.radius * 0.95).max(2.0);
        if self.inner_radius == inner_radius {
            return;
        }
        self.inner_radius = inner_radius;
        self.update_geometry();
        self.base.request_redraw();
    }

    /// Returns the center point of the menu.
    pub fn center(&self) -> Point {
        self.center
    }

    /// Sets the center point of the menu, in parent-relative logical pixels,
    /// and recomputes the widget geometry so it is the square of side
    /// `2 * radius` centred on that point.
    /// Moves the menu's centre, which also moves its rectangle.
    ///
    /// Repaints, for the same reason [`Self::set_radius`] does.
    pub fn set_center(&mut self, center: Point) {
        if self.center == center {
            return;
        }
        self.center = center;
        self.update_geometry();
        self.base.request_redraw();
    }

    /// Returns the animation progress (`0.0` to `1.0`).
    pub fn animation_progress(&self) -> f32 {
        self.animation_progress.value()
    }

    /// Sets the animation progress, clamped to `0.0 ..= 1.0`.
    ///
    /// The value is snapped rather than eased: a caller driving the reveal itself wants a
    /// definite picture, so both the position and the target are written (a driver that kept
    /// pointing at the previous target would animate away from where it was just placed).
    pub fn set_animation_progress(&mut self, progress: f32) {
        self.animation_progress.jump_to(progress.clamp(0.0, 1.0));
    }

    /// Advances the opening reveal by `delta_ms`, reporting whether another frame is needed.
    ///
    /// The target follows visibility: a shown menu opens to `1.0`, a hidden one closes to
    /// `0.0`. A menu that is hidden is not drawn at all, so the closed end is reached off
    /// screen and the value is ready at the resting end for the next `show_at` — which is what
    /// makes "open" start from nothing instead of from wherever the last close stopped.
    pub fn tick(&mut self, delta_ms: u32) -> bool {
        let target = if self.is_visible() { 1.0 } else { 0.0 };
        self.animation_progress.set_target(target);
        if self.animation_progress.tick(delta_ms) {
            self.base.request_redraw();
            return true;
        }
        // The settle frame is still paint-worthy: it is the frame that draws the open menu.
        self.animation_progress.is_moving()
    }

    /// Returns the hover highlight colour: the caller's override, or the theme-derived
    /// `Self::theme_hover_color` when none was set.
    pub fn hover_color(&self) -> Color {
        self.hover_color.unwrap_or_else(Self::theme_hover_color)
    }

    /// Sets the hover highlight colour, overriding the theme-derived default, and
    /// requests a redraw.
    pub fn set_hover_color(&mut self, color: Color) {
        if self.hover_color == Some(color) {
            return;
        }
        self.hover_color = Some(color);
        // Both colours are read by `draw` (the highlight fill, and the hovered label's contrast
        // colour), so without this the change was invisible until something else forced a frame.
        self.base.request_redraw();
    }

    /// Returns the label colour: the caller's override, or the theme-derived
    /// `Self::theme_text_color` when none was set.
    pub fn text_color(&self) -> Color {
        self.text_color.unwrap_or_else(Self::theme_text_color)
    }

    /// Sets the text colour for labels, overriding the theme-derived default, and
    /// requests a redraw.
    pub fn set_text_color(&mut self, color: Color) {
        if self.text_color == Some(color) {
            return;
        }
        self.text_color = Some(color);
        self.base.request_redraw();
    }

    /// The hover highlight an unset colour resolves to: the active theme's `primary`
    /// token, or the control's pre-theme blue when no theme is active.
    ///
    /// One derivation shared by [`Self::hover_color`], `draw` and the tests, so the
    /// getter cannot drift from the picture. It reads the cached palette, so it takes no manager
    /// lock (K-P17).
    fn theme_hover_color() -> Color {
        match crate::style::palette_shared() {
            Some(colors) => colors.primary,
            None => Color::rgb(0, 120, 215),
        }
    }

    /// The label ink an unset colour resolves to: the active theme's foreground token,
    /// or the control's pre-theme near-black when no theme is active.
    fn theme_text_color() -> Color {
        match crate::style::palette_shared() {
            Some(colors) => colors.foreground,
            None => Color::rgb(30, 30, 30),
        }
    }

    /// Returns the currently hovered item index, if any.
    pub fn hovered_index(&self) -> Option<usize> {
        self.hovered_index
    }

    /// Shows the menu centred on `center` (parent-relative logical pixels).
    ///
    /// Clears the hover highlight, emits `about_to_show`, then makes the widget
    /// visible. The radius and item list are unchanged.
    pub fn show_at(&mut self, center: Point) {
        self.center = center;
        self.submenu_path.clear();
        self.update_geometry();
        self.hovered_index = None;
        self.about_to_show.emit();
        self.base.show();
    }

    /// Hides the menu, clears the hover highlight, then emits `about_to_hide`.
    ///
    /// Note the ordering is the reverse of [`PieMenu::show_at`], which emits
    /// before showing. Emits unconditionally, even when already hidden.
    pub fn hide(&mut self) {
        self.base.hide();
        self.hovered_index = None;
        self.submenu_path.clear();
        self.about_to_hide.emit();
    }

    // ── Private helpers ──────────────────────────────────────────

    /// Recalculates the start/end angles for every item based on equal division.
    fn recalculate_angles(&mut self) {
        Self::distribute_angles(&mut self.items);
    }

    /// Divides a full circle evenly among `items`.
    fn distribute_angles(items: &mut [PieMenuItem]) {
        let count = items.len();
        if count == 0 {
            return;
        }
        let slice = TAU / count as f32;
        for (i, item) in items.iter_mut().enumerate() {
            item.set_angle_start(i as f32 * slice);
            item.set_angle_end((i as f32 + 1.0) * slice);
        }
    }

    /// The slots the menu is currently showing: the deepest ring on the open branch path, or the
    /// top-level items when no branch is open.
    fn active_items(&self) -> &[PieMenuItem] {
        let mut ring: &[PieMenuItem] = &self.items;
        for index in &self.submenu_path {
            let Some(item) = ring.get(*index) else {
                // A path entry that no longer addresses anything (the item list was rewritten)
                // stops the descent at the ring it could still resolve, rather than panicking or
                // silently showing the top level.
                break;
            };
            if item.children.is_empty() {
                break;
            }
            ring = &item.children;
        }
        ring
    }

    /// Mutable counterpart of [`Self::active_items`] for one slot in the shown ring.
    fn active_item_mut(&mut self, index: usize) -> Option<&mut PieMenuItem> {
        let mut ring: &mut [PieMenuItem] = &mut self.items;
        for path_index in self.submenu_path.iter() {
            let slot = ring.get_mut(*path_index)?;
            if slot.children.is_empty() {
                return None;
            }
            ring = &mut slot.children;
        }
        ring.get_mut(index)
    }

    /// How many rings are open below the top level; `0` while the top level is showing.
    pub fn submenu_depth(&self) -> usize {
        self.submenu_path.len()
    }

    /// The top-level slot whose child ring is showing, if any.
    pub fn open_submenu_index(&self) -> Option<usize> {
        self.submenu_path.first().copied()
    }

    /// Opens the child ring of the slot at `index` in the shown ring.
    ///
    /// Returns `false` when the slot has no children. The children's arcs are divided evenly
    /// among themselves, the same relation the top level uses. Any depth is allowed: opening a
    /// branch inside an open branch pushes another level rather than being refused.
    pub fn open_submenu(&mut self, index: usize) -> bool {
        let Some(item) = self.active_item(index) else {
            return false;
        };
        if item.children.is_empty() {
            return false;
        }
        if let Some(item) = self.active_item_mut(index) {
            Self::distribute_angles(&mut item.children);
        }
        self.submenu_path.push(index);
        self.hovered_index = None;
        self.base.request_redraw();
        true
    }

    /// Returns from a child ring to the ring above it. Returns whether a ring was open.
    pub fn close_submenu(&mut self) -> bool {
        if self.submenu_path.pop().is_none() {
            return false;
        }
        self.hovered_index = None;
        self.base.request_redraw();
        true
    }

    /// The slot at `index` in the currently shown ring.
    fn active_item(&self, index: usize) -> Option<&PieMenuItem> {
        self.active_items().get(index)
    }

    /// Moves the highlight by `delta` slots in the shown ring, skipping gaps and
    /// disabled slots.
    fn move_highlight(&mut self, delta: i32) {
        let count = self.active_items().len();
        if count == 0 || delta == 0 {
            return;
        }
        let start = match self.hovered_index {
            Some(index) => index as i32,
            None if delta > 0 => -1,
            None => count as i32,
        };
        let mut index = start;
        for _ in 0..count {
            index = (index + delta).rem_euclid(count as i32);
            if let Some(item) = self.active_item(index as usize) {
                if item.is_enabled() && !item.is_separator() {
                    self.hovered_index = Some(index as usize);
                    self.base.request_redraw();
                    return;
                }
            }
        }
    }

    /// Activates the slot at `index` in the shown ring.
    ///
    /// A branch opens its child ring and keeps the menu up; a leaf reports the choice
    /// and hides the menu.
    fn activate(&mut self, index: usize) -> bool {
        let Some(item) = self.active_item(index) else {
            return false;
        };
        if !item.is_enabled() || item.is_separator() {
            return false;
        }
        let is_branch = !item.children.is_empty();
        let text = item.text().to_string();
        let id = item.id();
        if is_branch {
            // A branch at any depth opens its own ring. The previous form only opened a ring
            // while none was open, so a grandchild branch was reported as a choice instead of
            // being entered.
            self.open_submenu(index);
            self.triggered_text.emit(text);
            self.triggered.emit(index);
            self.triggered_id.emit(id);
            return true;
        }
        self.triggered.emit(index);
        self.triggered_text.emit(text);
        self.triggered_id.emit(id);
        self.hide();
        true
    }

    /// Updates the widget geometry to match the current center and radius.
    fn update_geometry(&mut self) {
        let size = (self.radius * 2.0) as u32;
        self.base.set_geometry(Rect::new(
            self.center.x - self.radius as i32,
            self.center.y - self.radius as i32,
            size,
            size,
        ));
    }

    /// Returns the index of the slice at the given position, or `None`.
    fn hit_test(&self, pos: Point) -> Option<usize> {
        let dx = (pos.x - self.center.x) as f32;
        let dy = (pos.y - self.center.y) as f32;
        let dist = (dx * dx + dy * dy).sqrt();
        if dist < self.inner_radius || dist > self.radius {
            return None;
        }
        let mut angle = dy.atan2(dx);
        if angle < 0.0 {
            angle += TAU;
        }
        for (i, item) in self.active_items().iter().enumerate() {
            if item.is_separator() {
                continue;
            }
            if angle >= item.angle_start() && angle < item.angle_end() {
                if item.is_enabled() {
                    return Some(i);
                }
                return None;
            }
        }
        None
    }

    /// Samples a circle into a contour of `segments` points, wound by `clockwise`, appended to
    /// `points`; returns the `(start, end)` range of the appended points.
    ///
    /// One sample per two pixels of arc, bounded at both ends: the floor keeps a small radius
    /// round instead of polygonal, and the ceiling keeps a large one affordable (a full circle at
    /// the largest radius here is 256 points, against the ~2 300 one-pixel line commands the
    /// previous strip approximation emitted for the same shape).
    ///
    /// The first point is not repeated at the end — a contour is implicitly closed by the fill.
    /// The buffer is the caller's so a frame reuses one allocation instead of one per contour
    /// (K-P07); the arithmetic is what the collecting version did, so the points are the same.
    fn push_circle_contour(
        points: &mut Vec<Point>,
        center: Point,
        radius: f32,
        clockwise: bool,
    ) -> (usize, usize) {
        let start = points.len();
        let segments = (radius.abs() * TAU / 2.0).ceil().clamp(32.0, 256.0) as u32;
        let step = TAU / segments as f32 * if clockwise { -1.0 } else { 1.0 };
        for index in 0..segments {
            let angle = index as f32 * step;
            points.push(Point::from_f32(
                center.x as f32 + radius * angle.cos(),
                center.y as f32 + radius * angle.sin(),
            ));
        }
        (start, points.len())
    }

    /// Fills the whole donut track — the ring the slices are cut from.
    ///
    /// Painted before any slice, so an open menu with no items is still a visible ring rather
    /// than nothing at all.
    ///
    /// # Why this is one filled path and not a fan of circle strokes
    ///
    /// The track used to be approximated by drawing one-pixel circle strokes at a range of radii,
    /// one strip per **two** pixels of band (`strips = band * 0.5`). Every gap between two strips
    /// was therefore unpainted, so what reached the frame was a half-covered mesh whose holes
    /// showed the window behind: on the dark appearance — where the ring is only ~12 levels above
    /// the window — the disc read as a speckled, muddy blob. Two oppositely wound contours filled
    /// with the non-zero rule are the exact shape, with no gaps and no anti-aliased seams.
    fn fill_ring(
        &self,
        context: &mut RenderContext,
        center: Point,
        outer_r: f32,
        inner_r: f32,
        color: Color,
    ) {
        let inner = inner_r.max(0.0).min(outer_r.max(0.0));
        let outer = outer_r.max(0.0);
        if outer <= 0.0 {
            return;
        }
        let mut scratch = self.paint_scratch.borrow_mut();
        let scratch = &mut *scratch;
        scratch.points.clear();
        scratch.contours.clear();
        let outer_contour = Self::push_circle_contour(&mut scratch.points, center, outer, false);
        scratch.contours.push(outer_contour);
        if inner > 0.0 {
            // Opposite winding turns the inner circle into a hole: with the same winding the
            // non-zero rule would count both crossings and paint the hub as part of the ring.
            let hole = Self::push_circle_contour(&mut scratch.points, center, inner, true);
            scratch.contours.push(hole);
        }
        if let Err(error) = context.fill_path_non_zero(&scratch.points, &scratch.contours, color) {
            log::warn!("pie menu track could not be filled: {error:?}");
        }
    }

    /// Builds one wedge's contour into `path`.
    ///
    /// One closed contour — the outer arc, then the inner arc back — filled with the non-zero
    /// rule. The previous form was the same strip fan as the track, so each wedge was striped at
    /// the same two-pixel pitch and showed the track through its own gaps; the two radial edge
    /// strokes it needed to close the shape are part of this polygon and are gone with it.
    ///
    /// A slot that paints nothing (a separator, a degenerate sweep) leaves `path` empty.
    fn build_slice_path(
        center: Point,
        outer_r: f32,
        inner_r: f32,
        angle_start: f32,
        angle_end: f32,
        path: &mut SlicePath,
    ) {
        path.points.clear();
        path.contour = (0, 0);
        let delta_angle = angle_end - angle_start;
        let outer = outer_r.max(0.0);
        let inner = inner_r.max(0.0).min(outer);
        if outer <= 0.0 || !delta_angle.is_finite() || delta_angle == 0.0 {
            return;
        }

        // One sample per two pixels of the outer arc, bounded so a thin sliver still encloses an
        // area (a contour needs three points) and a large sweep stays affordable.
        let steps = (outer * delta_angle.abs() / 2.0).ceil().clamp(8.0, 256.0) as u32;
        for index in 0..=steps {
            let angle = angle_start + delta_angle * (index as f32 / steps as f32);
            path.points.push(Point::from_f32(
                center.x as f32 + outer * angle.cos(),
                center.y as f32 + outer * angle.sin(),
            ));
        }
        if inner > 0.0 {
            for index in (0..=steps).rev() {
                let angle = angle_start + delta_angle * (index as f32 / steps as f32);
                path.points.push(Point::from_f32(
                    center.x as f32 + inner * angle.cos(),
                    center.y as f32 + inner * angle.sin(),
                ));
            }
        } else {
            // A full pie closes at the centre instead of along an inner arc.
            path.points.push(center);
        }
        path.contour = (0, path.points.len());
    }

    /// Rebuilds every wedge's contour when the radius, the centre or an angle changed.
    ///
    /// The list is indexed exactly like `active_items()`, so the draw loop can read a slot's
    /// contour without searching for it. A separator gets an empty path.
    fn refresh_slice_paths(&self, center: Point, outer_r: f32, inner_r: f32) {
        let items = self.active_items();
        let key = SlicePathKey::of(center, outer_r, inner_r, items);
        let mut cache = self.slice_paths.borrow_mut();
        if cache.key == Some(key) {
            return;
        }
        cache.slices.clear();
        for item in items {
            let mut path = SlicePath::default();
            if !item.is_separator() {
                Self::build_slice_path(
                    center,
                    outer_r,
                    inner_r,
                    item.angle_start(),
                    item.angle_end(),
                    &mut path,
                );
            }
            cache.slices.push(path);
        }
        cache.key = Some(key);
    }

    /// The fill the enabled, un-highlighted wedges take: one visible step off the track
    /// ring, derived from the colours `draw` has already resolved.
    ///
    /// This used to cycle a fixed twelve-pastel palette (light blue, light pink, pale
    /// green, …). Those pastels are *menu chrome*, not data — a pie menu is a popup whose
    /// wedges are buttons, the same reading that lets the sibling `menu` paint its rows
    /// in the theme's surface and primary — so a dark window read as a ring of pastel
    /// wedges no theme could reach. The wedges now take a single tone derived from the
    /// themed ring and ink (the pair the track, hub and border derive from), which the
    /// separators and the `primary` hover highlight distinguish.
    fn slice_color(&self, ring: Color, ink: Color) -> Color {
        ring.blend(&ink, 0.08)
    }
}

impl Widget for PieMenu {
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
        crate::core::Size::new(
            dimensions::PIE_MENU_DEFAULT_WIDTH,
            dimensions::PIE_MENU_DEFAULT_HEIGHT,
        )
    }

    fn show(&mut self) {
        self.about_to_show.emit();
        self.base.show();
    }

    fn hide(&mut self) {
        self.base.hide();
        self.hovered_index = None;
        self.about_to_hide.emit();
    }

    /// Lifts the reveal onto the trait so the animation bus can reach it through `&mut dyn Widget`.
    fn tick(&mut self, delta_ms: u32) -> bool {
        PieMenu::tick(self, delta_ms)
    }

    fn is_animating(&self) -> bool {
        self.animation_progress.is_moving()
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
                CapabilityValue::UInt(*v as u64)
            })),
            "triggered_text" => {
                Some(EventSignalRef::mapped("triggered_text", &self.triggered_text, |v| {
                    CapabilityValue::String(v.clone())
                }))
            }
            "about_to_show" => Some(EventSignalRef::unit("about_to_show", &self.about_to_show)),
            "about_to_hide" => Some(EventSignalRef::unit("about_to_hide", &self.about_to_hide)),
            "triggered_id" => {
                Some(EventSignalRef::mapped("triggered_id", &self.triggered_id, |v| {
                    CapabilityValue::UInt(*v)
                }))
            }
            _ => None,
        }
    }
}

/// `PieMenu`'s property contract.
///
/// All four properties are read-only projections of the menu's geometry and
/// selection state — none of them has a setter that would not fight the layout
/// that produced the radii — so there is no write arm for them and the schema
/// marks each one non-writable.
impl WidgetProperties for PieMenu {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "items" => {
                Ok(CapabilityValue::Records(crate::widget::capability::RecordList::from_items(
                    &PIE_MENU_ITEM_SCHEMA,
                    self.items
                        .iter()
                        .map(pie_menu_item_record)
                        .collect::<Result<Vec<_>, CapabilityAccessError>>()?,
                )))
            }
            "item_count" => Ok(CapabilityValue::UInt(self.item_count() as u64)),
            "radius" => Ok(CapabilityValue::Float(self.radius() as f64)),
            "inner_radius" => Ok(CapabilityValue::Float(self.inner_radius() as f64)),
            "current_index" => Ok(CapabilityValue::UInt(self.selected_index().unwrap_or(0) as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "radius" => {
                self.set_radius(expect_f32(value)?);
                Ok(())
            }
            "inner_radius" => {
                self.set_inner_radius(expect_f32(value)?);
                Ok(())
            }
            "current_index" => {
                self.set_current_index(expect_usize(value)?);
                Ok(())
            }
            // Derived from the item vector, so there is nothing to assign.
            "item_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            // The rows carry identity and recomputed slice angles. The write rebuilds the flat
            // top-level ring through `add_existing_item`, which owns both normalisations — the id
            // counter skips a pinned id, and the angles are recomputed — rather than restating
            // either here. A row cannot express a child ring (`value_matches_kind` refuses a
            // nested record list), so a write declares the top level and any existing submenu
            // rings are replaced along with the rows they hung from.
            "items" => {
                let CapabilityValue::Records(list) = value else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                // Checked in full before anything is replaced, so a refused update leaves the live
                // items untouched rather than half-swapped.
                list.validate_against(&PIE_MENU_ITEM_SCHEMA).map_err(|violation| {
                    log::warn!("pie_menu.items refused: {violation}");
                    CapabilityAccessError::ConstraintViolation {
                        property: crate::compat::String::from("items"),
                        reason: crate::compat::format!("{violation}"),
                    }
                })?;
                let mut items = Vec::with_capacity(list.len());
                for record in list.items() {
                    items.push(pie_menu_item_from_record(record)?);
                }
                // Uniqueness is the list's rule and the schema cannot express it for a `UInt`
                // identity (its id reader is text-only), so it is enforced here before the swap.
                let mut seen = Vec::new();
                for item in &items {
                    if item.id() != 0 && seen.contains(&item.id()) {
                        return Err(CapabilityAccessError::ConstraintViolation {
                            property: crate::compat::String::from("items"),
                            reason: crate::compat::format!("duplicate id {} in items", item.id()),
                        });
                    }
                    seen.push(item.id());
                }
                self.items.clear();
                for item in items {
                    self.add_existing_item(item);
                }
                self.hovered_index = None;
                self.submenu_path.clear();
                self.base.request_redraw();
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "items",
            "item_count",
            "radius",
            "inner_radius",
            "current_index",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `pie_menu` publishes.
    ///
    /// `add_item` takes the label string and `remove_item` the index, so those
    /// are answered as needing a payload rather than being called unknown.
    /// `set_radius` is wired to the widget's real `set_radius`, which recomputes
    /// the geometry, but the value is what the caller actually wants to change,
    /// so a bare invocation is reported as needing it too. `set_current_index`
    /// selects the item at that index, which likewise cannot be guessed.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "add_item" | "remove_item" | "set_radius" | "set_current_index" | "set_items" => {
                Err(CapabilityAccessError::OutOfRange)
            }
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for PieMenu {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() || !self.base.is_visible() {
            return;
        }
        match event {
            Event::MouseMove { pos } => {
                // Hover is painted (the wedge under the pointer takes `hover_color`), so the field
                // and the repaint are one step — and only on an actual change, because `MouseMove`
                // fires far more often than the highlighted wedge changes. Without the repaint the
                // highlight did not appear until an unrelated event (a `tick`, or a re-show).
                let hovered = self.hit_test(*pos);
                if hovered != self.hovered_index {
                    self.hovered_index = hovered;
                    self.base.request_redraw();
                }
            }
            Event::MouseLeave { .. } => {
                if self.hovered_index.take().is_some() {
                    self.base.request_redraw();
                }
            }
            Event::MousePress { pos, button: 1, .. } => {
                if let Some(idx) = self.hit_test(*pos) {
                    self.activate(idx);
                }
            }
            Event::KeyPress { key, .. } => match *key {
                key_codes::ESCAPE => {
                    // Escape unwinds one ring at a time: a child ring closes first.
                    if !self.close_submenu() {
                        self.hide();
                    }
                }
                key_codes::LEFT | key_codes::UP => self.move_highlight(-1),
                key_codes::RIGHT | key_codes::DOWN => self.move_highlight(1),
                key_codes::HOME => {
                    self.hovered_index = None;
                    self.move_highlight(1);
                }
                key_codes::END => {
                    self.hovered_index = None;
                    self.move_highlight(-1);
                }
                key_codes::ENTER | key_codes::SPACE => {
                    if let Some(index) = self.hovered_index {
                        self.activate(index);
                    } else if let Some(index) = self.selected_index() {
                        self.activate(index);
                    }
                }
                _ => {}
            },
            _ => { /* Other events are not relevant */ }
        }
    }
}

/// The label box for the hub readout, centred on the hub and inscribed in the hub disc.
///
/// The box's own half diagonal stays inside the circle (`√(0.85² + 0.45²) ≈ 0.96 < 1` in
/// units of the hub radius), so a label the fitter keeps inside the box never crosses the
/// ring — the fitter elides it first.
fn hub_label_box(cx: f32, cy: f32, inner_r: f32) -> Rect {
    let box_w = (inner_r * 1.7).max(1.0) as u32;
    let box_h = (inner_r * 0.9).max(1.0) as u32;
    Rect::new(
        (cx - box_w as f32 / 2.0).round() as i32,
        (cy - box_h as f32 / 2.0).round() as i32,
        box_w,
        box_h,
    )
}

impl Draw for PieMenu {
    fn draw(&mut self, context: &mut RenderContext) {
        if !self.is_visible() {
            return;
        }

        // Chrome colours resolve the explicit style first, then the theme's resolved style for
        // this control, and only then a literal. The ring, its separators and the hub hole all
        // used to be literals, so a light/dark switch left the control's chrome unchanged — the
        // rendering census reported it as theme-blind.
        //
        // The style lookups are the cached, revision-validated forms: `resolved_theme_style_shared`
        // is a refcount bump on a hit and `palette_shared` answers the three colours this draw needs
        // in one read, so neither takes the manager lock (K-P07/K-P17).
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("pie_menu");
        let (window_fill, foreground, secondary) = match crate::style::palette_shared() {
            Some(colors) => (colors.background, colors.foreground, colors.secondary),
            None => (Color::rgb(240, 240, 240), Color::BLACK, Color::rgb(158, 158, 158)),
        };
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(foreground);
        // `pie_menu` is absent from `WidgetRole::for_kind_name`'s table, so it classifies as
        // `Surface` and the active theme writes the window fill into `style.background_color`.
        // A ring painted in that colour would be byte-identical to the frame behind it, so a
        // resolved surface equal to the window fill is re-derived a visible step away from it,
        // while a colour the caller set still wins.
        let ring = match style.background_color {
            Some(resolved) if resolved != window_fill => resolved,
            _ => window_fill.blend(&ink, 0.12),
        };
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .filter(|resolved| *resolved != ring)
            .unwrap_or_else(|| ring.blend(&secondary, 0.45));

        let center = self.center;
        // The reveal scales the ring's radii, so an opening menu grows into place instead of
        // appearing whole. The progress is read here rather than in the hit test: the *picture*
        // follows the animation, while the hit test keeps using the final geometry, so a pointer
        // that arrives mid-open cannot land on a wedge that is not there yet.
        let reveal = self.animation_progress.value();
        let cx = center.x as f32;
        let cy = center.y as f32;

        // A menu that is fully closed is still **drawn**, at its final geometry and at reduced
        // strength, rather than omitted.
        //
        // # Why this is the crate's convention and not a favour to the census
        //
        // A pie menu is constructed closed (`PropertyDriver::at(0.0)`), and this control used to
        // `return` outright at `reveal <= 0.0` — so a freshly built menu painted nothing at all.
        // That is the exact defect `popover` documents and fixes in the same words: "the whole
        // card used to be skipped unless the popover was already showing — so the census reported
        // `ink = 0` *and* no response to a light/dark switch", and its half of the comment above
        // ("a freshly constructed menu is visible rather than reporting `ink = 0`") was written
        // for the same reason but could never run, because this return sat in front of it.
        //
        // The three sibling popups agree: `menu` draws its heading when closed, `tooltip` blends
        // its bubble toward the window, and `popover` lays the card out and dims it 45 %. All three
        // draw a *visible body* in the closed state, which is what makes the appearance switch
        // observable before the thing is opened and what lets a reader see where it will appear.
        // So the closed menu paints its track ring dimmed, and the opening animation scales that
        // same ring up from it — the two states are the same object at two strengths, not one
        // object and one blank.
        let closed = reveal <= 0.0;
        let outer_r = if closed { self.radius } else { self.radius * reveal };
        let inner_r = if closed { self.inner_radius } else { self.inner_radius * reveal };
        if closed {
            self.fill_ring(context, center, outer_r, inner_r, window_fill.blend(&ring, 0.45));
            return;
        }

        // The ring track and its hub are drawn before the early return on an empty item list, so a
        // freshly constructed menu is visible rather than reporting `ink = 0`; an empty ring reads
        // as a menu that has been opened with nothing to offer.
        self.fill_ring(context, center, outer_r, inner_r, ring);

        // The highlight and the enabled wedges' fill resolve once per frame, before the loop:
        // `hover_color` takes the manager lock, which is not re-entrant, so it must not be
        // acquired per wedge (and per-wedge recomputation of a constant blend is waste).
        let hover_fill = self.hover_color();
        let wedge_fill = self.slice_color(ring, ink);
        // A static ring reuses the contours the previous frame built; only a radius, centre or
        // angle change rebuilds them. Hover and disabled state are colours, read below.
        self.refresh_slice_paths(center, outer_r, inner_r);
        // Draw each slice
        for (i, item) in self.active_items().iter().enumerate() {
            if item.is_separator() {
                // A separator is a gap in the ring: no fill, no label, no hit target.
                continue;
            }
            let is_hovered = self.hovered_index == Some(i);
            // A disabled slice takes the themed track — the fill an enabled wedge sits one
            // visible step above — so it reads as a muted wedge on either appearance.
            let base_color = if !item.is_enabled() {
                ring
            } else if is_hovered {
                hover_fill
            } else {
                wedge_fill
            };

            let cache = self.slice_paths.borrow();
            let Some(path) = cache.slices.get(i) else {
                continue;
            };
            if !path.points.is_empty() {
                if let Err(error) = context.fill_path_non_zero(
                    &path.points,
                    core::slice::from_ref(&path.contour),
                    base_color,
                ) {
                    log::warn!("pie menu slice {i} could not be filled: {error:?}");
                }
            }
        }

        // Draw separator lines between slices (radial lines)
        for item in self.active_items().iter() {
            if item.is_separator() {
                continue;
            }
            context.draw_line_stroke(
                Point::from_f32(
                    cx + inner_r * item.angle_start().cos(),
                    cy + inner_r * item.angle_start().sin(),
                ),
                Point::from_f32(
                    cx + outer_r * item.angle_start().cos(),
                    cy + outer_r * item.angle_start().sin(),
                ),
                border,
                1,
            );
        }

        // Draw the outer ring border
        context.draw_circle_stroke(center, outer_r as u32, border, 1);

        // Draw the inner donut hole circle
        let hub = ring.blend(&ink, 0.10);
        context.fill_circle(center, inner_r as u32, hub);
        context.draw_circle_stroke(center, inner_r as u32, border, 1);

        // Draw text labels centered in each slice, with the **effective font** — the resolved
        // theme/caller font — so the labels honour the theme body font and the user's text scale
        // (D09-STYLE-01).
        let font = effective_font(style);
        for (i, item) in self.active_items().iter().enumerate() {
            if !item.is_enabled() || item.is_separator() {
                continue;
            }
            let mid_angle = (item.angle_start() + item.angle_end()) * 0.5;
            let label_r = (outer_r + inner_r) * 0.5;
            let lx = cx + label_r * mid_angle.cos();
            let ly = cy + label_r * mid_angle.sin();

            // A highlighted label takes the contrast colour of the accent it sits on, so it is not
            // a fixed white that may vanish on a light accent. An unset label colour follows the
            // same ink the ring's chrome resolves, so labels move with the theme instead of a
            // fixed near-black that only reads on light wedges.
            let text_color = if self.hovered_index == Some(i) {
                hover_fill.contrast_color()
            } else {
                self.text_color.unwrap_or(ink)
            };
            if let Some(source) = item.icon_source() {
                let icon_rect =
                    Rect::new((lx - 12.0).round() as i32, (ly - 12.0).round() as i32, 24, 24);
                if let Err(error) =
                    crate::widget::paint_icon(context, source.into(), icon_rect, text_color)
                {
                    log::warn!("pie menu item {i} icon failed: {error:?}");
                }
            } else {
                let label_text =
                    if item.icon_text().is_empty() { item.text() } else { item.icon_text() };
                // The label is **centred on the wedge**, in a box bounded by the wedge's own
                // geometry: the chord at the label radius is what two neighbouring labels may
                // share, and the radial band is how tall the run can be. The previous call used
                // the wedge centre as a *left* origin, so every label began at its wedge's middle
                // and ran outward — the two top labels met over the hub and the right one left the
                // control. Fitting the box also elides a label that genuinely cannot fit instead
                // of overlapping its neighbour.
                let sweep = (item.angle_end() - item.angle_start()).abs();
                let half_chord = (label_r * (sweep * 0.5).sin()).abs();
                // `draw_text_fitted` insets both ends by `TEXT_FIT_MARGIN`, so a box sized to the
                // bare chord asks it to fit the label into 6 px less than the chord — which
                // elided `Rename` to `Rena…` in a wedge that had room for the whole word.
                let box_w = ((half_chord * 2.0) + 2.0 * TEXT_FIT_MARGIN as f32).max(1.0) as u32;
                let box_h = ((outer_r - inner_r) * 0.8).max(1.0) as u32;
                let label_box = Rect::new(
                    (lx - box_w as f32 / 2.0).round() as i32,
                    (ly - box_h as f32 / 2.0).round() as i32,
                    box_w,
                    box_h,
                );
                context.draw_text_fitted_cow(
                    label_box,
                    label_text,
                    font,
                    text_color,
                    HorizontalAlignment::Center,
                );
            }
            // The shortcut hint sits nearer the rim than the label, in the weaker ink: it
            // annotates the slot rather than naming it.
            if let Some(shortcut) = item.shortcut() {
                let hint_r = (outer_r - 10.0).max(inner_r);
                let hx = cx + hint_r * mid_angle.cos();
                let hy = cy + hint_r * mid_angle.sin();
                context.draw_text_fitted_cow(
                    Rect::new((hx - 24.0) as i32, (hy - 8.0) as i32, 48, 16),
                    shortcut,
                    font,
                    secondary,
                    HorizontalAlignment::Center,
                );
            }
        }

        // The hovered item's name reads out in the hub. A slice led by a typed icon paints
        // no label at all, so the hub is where the name becomes legible — the readout
        // weapon-wheel menus use. The idle hub keeps its centre dot.
        let hub_label = self
            .hovered_index
            .and_then(|i| self.active_item(i))
            .filter(|item| item.is_enabled())
            .map(|item| item.text());
        if let Some(label) = hub_label {
            let label_box = hub_label_box(cx, cy, inner_r);
            context.draw_text_fitted_cow(
                label_box,
                label,
                font,
                self.text_color.unwrap_or(ink),
                HorizontalAlignment::Center,
            );
        } else {
            // Draw a small center dot
            context.fill_circle(center, 3, border);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Color;
    use crate::event::Event;
    use crate::widget::svg::render_to_svg;
    use std::sync::{Arc, Mutex};

    /// 1. Creating default PieMenu (verify kind, geometry, default state)
    #[test]
    fn test_default_creation() {
        let center = Point::new(200, 200);
        let radius = 100.0;
        let menu = PieMenu::new(center, radius);

        assert_eq!(menu.kind(), WidgetKind::PieMenu);
        assert_eq!(menu.center(), center);
        assert_eq!(menu.radius(), radius);
        assert!(menu.inner_radius() < radius);
        assert_eq!(menu.inner_radius(), radius * 0.35);
        assert_eq!(menu.selected_index().unwrap_or(0), 0);
        assert_eq!(menu.hovered_index(), None);
        // At the **resting** end: a freshly built menu has not been opened yet, so it starts
        // closed and unfurls. The previous expectation (`1.0`) was the shape BLUE24 §2.4 gate B
        // forbids -- a control constructed already finished, which makes `show_at` a no-op
        // animation and hides the reveal entirely.
        assert_eq!(menu.animation_progress(), 0.0);
        assert_eq!(menu.item_count(), 0);
        assert!(menu.is_visible());
        assert!(menu.is_enabled());
        assert!(menu.items().is_empty());

        // Default colours follow the active theme — the fixed literals only when no theme is
        // active — so the assertions are anchored to the same derivations the getters use and
        // hold whichever appearance the process happens to be in.
        assert_eq!(menu.hover_color(), PieMenu::theme_hover_color());
        assert_eq!(menu.text_color(), PieMenu::theme_text_color());

        // Geometry: centered at (200,200) with radius 100 => rect (100, 100, 200, 200)
        let geom = menu.geometry();
        assert_eq!(geom.x, 100);
        assert_eq!(geom.y, 100);
        assert_eq!(geom.width, 200);
        assert_eq!(geom.height, 200);
    }

    /// 2. Adding items
    #[test]
    fn test_add_item() {
        let mut menu = PieMenu::new(Point::new(100, 100), 80.0);

        // Each append returns the id it assigned. Ids start at 1 because an item that arrives
        // without one cannot be told from an item whose id *is* 0 (see `PieMenuItem::new`).
        let idx0 = menu.add_item("Cut");
        assert_eq!(idx0, 1);
        assert_eq!(menu.item_count(), 1);

        let idx1 = menu.add_item("Copy");
        assert_eq!(idx1, 2);
        assert_eq!(menu.item_count(), 2);

        let idx2 = menu.add_item_with_icon("Paste", "📋");
        assert_eq!(idx2, 3);
        assert_eq!(menu.item_count(), 3);

        // Verify items
        let items = menu.items();
        assert_eq!(items[0].text(), "Cut");
        assert_eq!(items[1].text(), "Copy");
        assert_eq!(items[2].text(), "Paste");
        assert_eq!(items[2].icon_text(), "📋");
    }

    #[test]
    /// The `items` row is the document channel: it carries the item's identity and its typed icon,
    /// which is what lets a designer round-trip a per-item icon for this collection (blue27 §19.10,
    /// and the U5 gap the exemption table recorded while these items had no identity).
    fn the_items_record_carries_identity_and_the_typed_icon() {
        use crate::widget::capability::{widget_property_get, CapabilityValue};

        let mut menu = PieMenu::new(Point::new(100, 100), 80.0);
        let first = menu.add_item("Cut");
        menu.add_item_with_icon("Copy", "📋");
        menu.set_item_icon_source(first, Some(IconSource::Builtin(crate::widget::IconName::Save)))
            .expect("the item exists");

        let rows = widget_property_get(&menu, "items").expect("`items` is readable");
        let CapabilityValue::Records(list) = rows else {
            panic!("`items` must read as a record list, got {rows:?}");
        };
        assert_eq!(list.len(), 2, "one row per item");

        let row = list.get(0).expect("the first item has a row");
        assert_eq!(row.get("id"), Some(&CapabilityValue::UInt(first)), "the row carries the id");
        assert_eq!(row.get("text"), Some(&CapabilityValue::String(String::from("Cut"))));
        assert!(matches!(
            row.get("icon_source"),
            Some(CapabilityValue::IconSpec(crate::widget::icon_source::IconSpec::Builtin(
                crate::widget::IconName::Save
            )))
        ));
        // The row's id is the same identity the setter takes, so a document's value addresses the
        // item directly rather than by a position that an edit could move.
        assert_eq!(
            menu.item_icon_source(first),
            Some(IconSource::Builtin(crate::widget::IconName::Save))
        );

        // A row for an item with no typed icon omits the field rather than reporting `null`, so
        // "no icon" and "cleared" stay distinguishable.
        let second = list.get(1).expect("the second item has a row");
        assert_eq!(second.get("icon_source"), None);
    }

    /// A source with no document form refuses the read instead of dropping the icon.
    #[test]
    fn the_items_record_refuses_a_source_with_no_document_form() {
        use crate::widget::capability::{widget_property_get, CapabilityAccessError};

        static RING: crate::widget::IconData = crate::widget::IconData {
            name: "row-ring",
            grid: 24,
            paths: &["M2-2H22V-22H2Z M7-7V-17H17V-7Z"],
        };

        let mut menu = PieMenu::new(Point::new(100, 100), 80.0);
        let id = menu.add_item("Ring");
        menu.set_item_icon_source(id, Some(IconSource::static_geometry(&RING)))
            .expect("the item exists");
        assert_eq!(
            widget_property_get(&menu, "items"),
            Err(CapabilityAccessError::IconSourceNotPersistable),
            "a runtime-only source must refuse the row, not vanish from it"
        );
    }

    /// The row list is writable: the write rebuilds the ring through `add_existing_item`, which
    /// recomputes the slice angles and advances the id counter past a pinned id.
    #[test]
    fn the_items_record_is_writable() {
        use crate::widget::capability::{widget_property_get, widget_property_set};

        let mut menu = PieMenu::new(Point::new(100, 100), 80.0);
        menu.add_item("Cut");
        let rows = widget_property_get(&menu, "items").expect("`items` is readable");
        assert_eq!(widget_property_set(&mut menu, "items", rows), Ok(()));
        assert_eq!(menu.item_count(), 1);
    }

    /// The row survives the JSON round trip the designer performs: rows -> document -> rows.
    #[test]
    fn the_items_row_round_trips_through_the_document_form() {
        use crate::widget::capability::{widget_property_get, CapabilityValue};

        let mut menu = PieMenu::new(Point::new(100, 100), 80.0);
        let id = menu.add_item_with_id(7, "Save").expect("a fresh id is accepted");
        menu.set_item_icon_source(id, Some(IconSource::Builtin(crate::widget::IconName::Save)))
            .expect("the item exists");

        let rows = widget_property_get(&menu, "items").expect("`items` is readable");
        let json = crate::json::records_to_json(&rows).expect("the rows have a document form");
        let text = serde_json::to_string(&json).expect("a record array serialises");
        assert!(
            text.contains(r#""kind":"builtin""#) && text.contains(r#""name":"save""#),
            "the typed icon must reach the document in its own form, got {text}"
        );
        assert!(text.contains(r#""id":7"#), "the identity must reach the document, got {text}");

        // The same rows read back as the same rows: the document is a faithful carrier.
        let again = crate::json::records_to_json(&rows).expect("still a document form");
        assert_eq!(json, again);
        assert!(matches!(rows, CapabilityValue::Records(_)));
    }

    #[test]
    fn pie_menu_typed_item_icon_tracks_collection_edits() {
        static RING: crate::widget::IconData = crate::widget::IconData {
            name: "pie-ring",
            grid: 24,
            paths: &["M2-2H22V-22H2Z M7-7V-17H17V-7Z"],
        };

        let mut menu = PieMenu::new(Point::new(100, 100), 80.0);
        menu.add_item("First");
        let second = menu.add_item("Second");
        menu.set_item_icon_source(second, Some(IconSource::static_geometry(&RING)))
            .expect("existing item accepts typed source");

        // The assignment follows the item it was made for, not the position it happened to occupy:
        // inserting ahead of it moves the index, and the icon must move *with the item* rather than
        // stay on the position (blue27 §19.6 — an index -> icon side table is what allows the
        // icon to silently land on a different item).
        menu.insert_item(0, "Inserted");
        assert_eq!(
            menu.item_icon_source(second),
            Some(IconSource::static_geometry(&RING)),
            "the inserted item must not inherit the earlier assignment"
        );
        assert_eq!(menu.item_icon_source(1), None, "only the assigned item carries the icon");

        assert_eq!(
            menu.set_item_icon_source(99, None),
            Err(IconItemError::ItemNotFound { id: 99 })
        );
        assert!(render_to_svg(&mut menu).starts_with("<svg"));

        menu.remove_item(0);
        assert_eq!(
            menu.item_icon_source(second),
            Some(IconSource::static_geometry(&RING)),
            "removing an earlier item must not move the icon to another item"
        );

        // A pinned id is what a document carries, and it must not be handed out twice.
        assert_eq!(
            menu.add_item_with_id(second, "Duplicate"),
            Err(IconItemError::AmbiguousItemId { id: second })
        );
        let pinned = menu.add_item_with_id(42, "Pinned").expect("a fresh id is accepted");
        assert_eq!(pinned, 42);
        assert_eq!(menu.item_icon_source(42), None);

        menu.clear();
        assert_eq!(
            menu.set_item_icon_source(second, None),
            Err(IconItemError::ItemNotFound { id: second })
        );
    }

    /// 3. Inserting items at specific index
    #[test]
    fn test_insert_item_at_index() {
        let mut menu = PieMenu::new(Point::new(100, 100), 80.0);
        menu.add_item("First");
        menu.add_item("Third");
        assert_eq!(menu.item_count(), 2);

        // Insert at middle
        menu.insert_item(1, "Second");
        assert_eq!(menu.item_count(), 3);
        assert_eq!(menu.items()[0].text(), "First");
        assert_eq!(menu.items()[1].text(), "Second");
        assert_eq!(menu.items()[2].text(), "Third");

        // Insert at beginning
        menu.insert_item(0, "Zero");
        assert_eq!(menu.items()[0].text(), "Zero");
        assert_eq!(menu.items()[1].text(), "First");

        // Insert beyond end => appends
        menu.insert_item(99, "Last");
        assert_eq!(menu.items().last().unwrap().text(), "Last");

        // Verify angles are recalculated (all items get equal slices)
        let count = menu.item_count();
        let slice = std::f32::consts::TAU / count as f32;
        for (i, item) in menu.items().iter().enumerate() {
            let expected_start = i as f32 * slice;
            let expected_end = (i as f32 + 1.0) * slice;
            assert!((item.angle_start() - expected_start).abs() < 0.001);
            assert!((item.angle_end() - expected_end).abs() < 0.001);
        }
    }

    /// 4. Removing items
    #[test]
    fn test_remove_item() {
        let mut menu = PieMenu::new(Point::new(100, 100), 80.0);
        menu.add_item("A");
        menu.add_item("B");
        menu.add_item("C");
        assert_eq!(menu.item_count(), 3);

        // Remove middle
        menu.remove_item(1);
        assert_eq!(menu.item_count(), 2);
        assert_eq!(menu.items()[0].text(), "A");
        assert_eq!(menu.items()[1].text(), "C");

        // Remove out of bounds => no-op
        menu.remove_item(99);
        assert_eq!(menu.item_count(), 2);

        // Remove all
        menu.remove_item(1);
        menu.remove_item(0);
        assert_eq!(menu.item_count(), 0);
    }

    /// 5. Setting/getting current index
    #[test]
    fn test_current_index() {
        let mut menu = PieMenu::new(Point::new(100, 100), 80.0);
        menu.add_item("Red");
        menu.add_item("Green");
        menu.add_item("Blue");

        // Default is 0
        assert_eq!(menu.selected_index().unwrap_or(0), 0);

        menu.set_current_index(1);
        assert_eq!(menu.selected_index().unwrap_or(0), 1);

        menu.set_current_index(2);
        assert_eq!(menu.selected_index().unwrap_or(0), 2);

        // Out of bounds => no-op
        menu.set_current_index(99);
        assert_eq!(menu.selected_index().unwrap_or(0), 2);
    }

    /// 6. Item count
    #[test]
    fn test_item_count() {
        let mut menu = PieMenu::new(Point::new(100, 100), 80.0);
        assert_eq!(menu.item_count(), 0);

        menu.add_item("One");
        assert_eq!(menu.item_count(), 1);

        menu.add_item("Two");
        menu.add_item("Three");
        assert_eq!(menu.item_count(), 3);

        menu.clear();
        assert_eq!(menu.item_count(), 0);
    }

    /// 7. Setting item text and icon
    #[test]
    fn test_item_text_and_icon() {
        let mut menu = PieMenu::new(Point::new(100, 100), 80.0);
        menu.add_item("Initial");

        // Mutable access via items() slice is not directly mutable,
        // but we can use the item API through the menu.
        // Create a new item and check PieMenuItem's setters
        let mut item = PieMenuItem::new("Test");
        assert_eq!(item.text(), "Test");
        assert_eq!(item.icon_text(), "");

        item.set_text("Updated");
        assert_eq!(item.text(), "Updated");

        item.set_icon_text("🚀");
        assert_eq!(item.icon_text(), "🚀");

        // Also verify that add_item_with_icon sets icon correctly
        let mut menu2 = PieMenu::new(Point::new(50, 50), 60.0);
        menu2.add_item_with_icon("Save", "💾");
        assert_eq!(menu2.items()[0].text(), "Save");
        assert_eq!(menu2.items()[0].icon_text(), "💾");
    }

    /// 8. Enabling/disabling items
    #[test]
    fn test_enable_disable_items() {
        let mut menu = PieMenu::new(Point::new(100, 100), 80.0);
        menu.add_item("Alpha");
        menu.add_item("Beta");
        menu.add_item("Gamma");

        // All start enabled
        assert!(menu.items()[0].is_enabled());
        assert!(menu.items()[1].is_enabled());
        assert!(menu.items()[2].is_enabled());

        menu.set_item_enabled(1, false);
        assert!(menu.items()[0].is_enabled());
        assert!(!menu.items()[1].is_enabled());
        assert!(menu.items()[2].is_enabled());

        // Re-enable
        menu.set_item_enabled(1, true);
        assert!(menu.items()[1].is_enabled());

        // Out of bounds => no-op
        menu.set_item_enabled(99, false);
        assert_eq!(menu.item_count(), 3);
    }

    /// 9. Setting radius and center
    #[test]
    fn test_radius_and_center() {
        let mut menu = PieMenu::new(Point::new(100, 100), 80.0);
        assert_eq!(menu.radius(), 80.0);
        assert_eq!(menu.center(), Point::new(100, 100));

        // Set radius
        menu.set_radius(120.0);
        assert_eq!(menu.radius(), 120.0);
        // Clamped minimum
        menu.set_radius(5.0);
        assert_eq!(menu.radius(), 10.0);

        // Set center
        menu.set_center(Point::new(300, 400));
        assert_eq!(menu.center(), Point::new(300, 400));

        // Geometry updates with new center/radius
        let geom = menu.geometry();
        assert_eq!(geom.x, 290); // center.x(300) - radius(10) = 290
        assert_eq!(geom.width, 20);
        assert_eq!(geom.height, 20);
    }

    /// 10. Item visibility (via base widget delegation)
    #[test]
    fn test_item_visibility() {
        let mut menu = PieMenu::new(Point::new(100, 100), 80.0);
        assert!(menu.is_visible());

        menu.hide();
        assert!(!menu.is_visible());
        assert_eq!(menu.hovered_index(), None);

        menu.show();
        assert!(menu.is_visible());

        // show_at also makes it visible
        menu.hide();
        menu.show_at(Point::new(200, 200));
        assert!(menu.is_visible());
    }

    /// 11. Signal accessors (triggered, triggered_text, about_to_show, about_to_hide)
    #[test]
    fn test_signal_accessors() {
        let mut menu = PieMenu::new(Point::new(100, 100), 80.0);
        menu.add_item("Open");
        menu.add_item("Save");
        menu.add_item("Exit");

        // Test triggered signal via set_current_index
        let triggered = Arc::new(Mutex::new(None));
        menu.triggered.connect({
            let triggered = Arc::clone(&triggered);
            move |val: Arc<usize>| {
                *triggered.lock().unwrap() = Some(*val);
            }
        });

        menu.set_current_index(1);
        assert_eq!(*triggered.lock().unwrap(), Some(1));

        // Test triggered_text signal
        let triggered_text = Arc::new(Mutex::new(None::<String>));
        menu.triggered_text.connect({
            let triggered_text = Arc::clone(&triggered_text);
            move |val: Arc<String>| {
                *triggered_text.lock().unwrap() = Some(val.to_string());
            }
        });

        menu.set_current_index(2);
        assert_eq!(triggered_text.lock().unwrap().as_deref(), Some("Exit"));

        // Test about_to_show signal
        let show_fired = Arc::new(Mutex::new(false));
        menu.about_to_show.connect({
            let show_fired = Arc::clone(&show_fired);
            move || {
                *show_fired.lock().unwrap() = true;
            }
        });

        menu.hide();
        menu.show_at(Point::new(150, 150));
        assert!(*show_fired.lock().unwrap());

        // Test about_to_hide signal
        let hide_fired = Arc::new(Mutex::new(false));
        menu.about_to_hide.connect({
            let hide_fired = Arc::clone(&hide_fired);
            move || {
                *hide_fired.lock().unwrap() = true;
            }
        });

        menu.hide();
        assert!(*hide_fired.lock().unwrap());
    }

    /// 12. Geometry delegation
    #[test]
    fn test_geometry_delegation() {
        let mut menu = PieMenu::new(Point::new(50, 60), 40.0);

        // Geometry via Widget trait
        let geom = menu.geometry();
        assert_eq!(geom.x, 10); // 50 - 40
        assert_eq!(geom.y, 20); // 60 - 40
        assert_eq!(geom.width, 80);
        assert_eq!(geom.height, 80);

        // set_geometry through Widget trait
        menu.set_geometry(Rect::new(0, 0, 100, 100));
        assert_eq!(menu.geometry(), Rect::new(0, 0, 100, 100));

        // rect() alias
        assert_eq!(menu.geometry(), Rect::new(0, 0, 100, 100));

        // position / size
        assert_eq!(menu.position(), Point::new(0, 0));
        assert_eq!(menu.size(), crate::core::Size::new(100, 100));
    }

    /// 13. Widget ID and kind
    #[test]
    fn test_widget_id_and_kind() {
        let menu = PieMenu::new(Point::new(0, 0), 50.0);

        // Kind
        assert_eq!(menu.kind(), WidgetKind::PieMenu);

        // ID should be non-zero (Object generates unique IDs)
        assert_ne!(menu.id(), 0);

        // Two menus should have different IDs
        let menu2 = PieMenu::new(Point::new(10, 10), 30.0);
        assert_ne!(menu.id(), menu2.id());
    }

    /// 14. SVG output verification
    #[test]
    fn test_svg_output() {
        let mut menu = PieMenu::new(Point::new(50, 50), 50.0);
        menu.add_item("Cut");
        menu.add_item("Copy");
        menu.add_item("Paste");
        // A menu opens from the resting end, so it has to be opened (and advanced to the end of
        // the reveal) before there is a picture to snapshot. This is the same order a host uses:
        // `show_at`, then frames until `tick` reports settled (BLUE24 §2.3's three-frame
        // criterion form).
        menu.show_at(Point::new(50, 50));
        while menu.tick(1000) {}

        let svg = render_to_svg(&mut menu);

        // Should be a valid SVG string
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains("xmlns=\"http://www.w3.org/2000/svg\""));

        // Geometry-derived viewBox/viewport
        assert!(svg.contains("width=\"100\""));
        assert!(svg.contains("height=\"100\""));

        // Should draw circle borders and center dot
        assert!(svg.contains("circle"));
        assert!(svg.contains("line"));

        // A closed menu draws nothing at all, which is the resting state rather than an error:
        // the reveal scales the ring down to zero, so any host that has not opened it sees an
        // empty frame instead of a menu that was never asked for. Asserted so the two ends are
        // both pinned and the `reveal <= 0.0` early return cannot silently become "draw the
        // full menu anyway".
        let mut closed = PieMenu::new(Point::new(10, 10), 10.0);
        closed.add_item("Cut");
        let closed_svg = render_to_svg(&mut closed);
        assert!(closed_svg.starts_with("<svg"));
        assert!(!closed_svg.contains("circle"), "a closed menu draws no ring");
    }

    /// The track and the wedges are **filled shapes**, not a mesh of one-pixel strokes.
    ///
    /// The fills used to be approximated by one circle stroke per two pixels of band, so half of
    /// every ring was unpainted and the window showed through the gaps — on the dark appearance
    /// the disc read as a speckled, muddy mesh (2305 `<line>` elements for the census snapshot).
    /// This pins the shape rather than the look: a handful of fills, and lines only for the
    /// separators and the two borders.
    #[cfg(not(alloc_frugal))]
    #[test]
    fn the_ring_and_its_wedges_are_filled_shapes() {
        let _theme_guard = crate::style::theme_test_guard();
        let mut menu = PieMenu::new(Point::new(50, 50), 40.0);
        for label in ["Cut", "Copy", "Paste"] {
            menu.add_item(label);
        }
        menu.show_at(Point::new(50, 50));
        while menu.tick(1000) {}

        let svg = render_to_svg(&mut menu);
        let paths = svg.matches("<path").count();
        let lines = svg.matches("<line").count();

        assert!(
            paths >= 4,
            "the track and three wedges are filled paths, not strokes: {paths} paths\n{svg}"
        );
        assert!(
            lines <= 12,
            "only the separators and the two border circles may be lines: {lines}\n{svg}"
        );
    }

    /// The hovered item's name reads out in the hub, and an icon-led wedge paints no label.
    ///
    /// The SVG backend emits text as glyph geometry (never a `<text>` element), so the
    /// readout is pinned relationally: hovering an icon-led slice adds glyph paths and
    /// retires the centre dot's circle, and leaving restores the idle frame exactly.
    #[cfg(not(alloc_frugal))]
    #[test]
    fn the_hovered_item_reads_out_in_the_hub() {
        let _theme_guard = crate::style::theme_test_guard();
        let mut menu = PieMenu::new(Point::new(100, 100), 80.0);
        let save = menu.add_item("Save document");
        menu.set_item_icon_source(save, Some(IconSource::Builtin(crate::widget::IconName::Save)))
            .expect("the item exists");
        let copy = menu.add_item("Copy files");
        menu.set_item_icon_source(copy, Some(IconSource::Builtin(crate::widget::IconName::Save)))
            .expect("the item exists");
        menu.show_at(Point::new(100, 100));
        while menu.tick(1000) {}

        let resting = render_to_svg(&mut menu);
        let resting_paths = resting.matches("<path").count();
        let resting_circles = resting.matches("<circle").count();

        // Hovering an icon-led slice (its angular bisector) adds the hub readout's glyphs
        // and retires the centre dot.
        menu.handle_event(&Event::MouseMove { pos: Point::new(100, 154) });
        assert_eq!(menu.hovered_index(), Some(0));
        while menu.tick(1000) {}
        let hovered = render_to_svg(&mut menu);
        let hovered_paths = hovered.matches("<path").count();
        let hovered_circles = hovered.matches("<circle").count();
        assert!(
            hovered_paths > resting_paths,
            "the hub gains the hovered name's glyphs: {resting_paths} -> {hovered_paths}\n{hovered}"
        );
        assert_eq!(
            hovered_circles + 1,
            resting_circles,
            "the centre dot yields to the readout: {resting_circles} -> {hovered_circles}\n{hovered}"
        );

        // Leaving returns the hub to the idle dot: byte-for-byte the resting frame.
        menu.handle_event(&Event::MouseLeave { pos: Point::new(0, 0) });
        while menu.tick(1000) {}
        let left = render_to_svg(&mut menu);
        assert_eq!(left, resting, "leaving clears the hub readout and restores the idle frame");
    }

    /// The hub readout's box is inscribed in the hub disc, so a label the fitter keeps
    /// inside the box also stays inside the circle.
    #[test]
    fn the_hub_label_box_fits_inside_the_hub_disc() {
        for inner_r in [4.0_f32, 12.0, 28.0, 80.0, 200.0] {
            let r = hub_label_box(0.0, 0.0, inner_r);
            let half_w = r.width as f32 / 2.0;
            let half_h = r.height as f32 / 2.0;
            assert!(
                half_w.hypot(half_h) <= inner_r + 0.5,
                "box corner {half_w},{half_h} leaves the disc r={inner_r}"
            );
            assert!(
                (r.x as f32 + half_w).abs() <= 1.0 && (r.y as f32 + half_h).abs() <= 1.0,
                "box is not centred on the hub: r={inner_r}, rect={r:?}"
            );
        }
    }

    /// 15. Disabled state blocks events
    #[test]
    fn test_disabled_state_blocks_events() {
        let mut menu = PieMenu::new(Point::new(100, 100), 80.0);
        menu.add_item("Test");

        // Disable the widget
        menu.set_enabled(false);
        assert!(!menu.is_enabled());

        // Hover events should not update hovered_index when disabled
        menu.handle_event(&Event::MouseMove { pos: Point::new(100, 100) });
        assert_eq!(menu.hovered_index(), None);

        // Mouse press should not trigger
        let triggered = Arc::new(Mutex::new(false));
        menu.triggered.connect({
            let triggered = Arc::clone(&triggered);
            move |_: Arc<usize>| {
                *triggered.lock().unwrap() = true;
            }
        });
        menu.handle_event(&Event::MousePress {
            pos: Point::new(100, 100),
            button: 1,
            modifiers: 0,
        });
        assert!(!*triggered.lock().unwrap());

        // Re-enable and verify events flow again
        // Use a point within the pie slice (dist >= inner_radius and <= radius)
        menu.set_enabled(true);
        menu.handle_event(&Event::MouseMove { pos: Point::new(150, 100) });
        assert_eq!(menu.hovered_index(), Some(0));
    }

    /// 16. Clear all items
    #[test]
    fn test_clear_items() {
        let mut menu = PieMenu::new(Point::new(100, 100), 80.0);
        menu.add_item("A");
        menu.add_item("B");
        menu.add_item("C");
        menu.set_current_index(1);

        menu.clear();
        assert_eq!(menu.item_count(), 0);
        assert!(menu.items().is_empty());
        assert_eq!(menu.hovered_index(), None);
    }

    /// 17. Inner radius get/set
    #[test]
    fn test_inner_radius() {
        let mut menu = PieMenu::new(Point::new(100, 100), 100.0);

        // Default
        assert!((menu.inner_radius() - 35.0).abs() < 0.001);

        // Set inner radius
        menu.set_inner_radius(50.0);
        assert!((menu.inner_radius() - 50.0).abs() < 0.001);

        // Clamped to min 2.0
        menu.set_inner_radius(1.0);
        assert!((menu.inner_radius() - 2.0).abs() < 0.001);

        // Clamped to max radius * 0.95
        menu.set_inner_radius(200.0);
        assert!((menu.inner_radius() - 95.0).abs() < 0.001);
    }

    /// 18. show_at and hide
    #[test]
    fn test_show_at_and_hide() {
        let mut menu = PieMenu::new(Point::new(0, 0), 50.0);

        // Initially visible
        assert!(menu.is_visible());

        // show_at updates center and makes visible
        menu.show_at(Point::new(200, 300));
        assert_eq!(menu.center(), Point::new(200, 300));
        assert!(menu.is_visible());
        assert_eq!(menu.hovered_index(), None);

        // hide
        menu.hide();
        assert!(!menu.is_visible());
        assert_eq!(menu.hovered_index(), None);

        // Widget trait show/hide delegation
        menu.show();
        assert!(menu.is_visible());
        menu.hide();
        assert!(!menu.is_visible());
    }

    /// 19. Hover color and text color
    ///
    /// An unset colour follows the theme derivation; an explicit colour wins over it —
    /// the split every themed colour field in the crate now takes.
    #[test]
    fn test_hover_and_text_colors() {
        let mut menu = PieMenu::new(Point::new(100, 100), 80.0);

        // An unset colour resolves through the theme derivation...
        assert_eq!(menu.hover_color(), PieMenu::theme_hover_color());
        assert_eq!(menu.text_color(), PieMenu::theme_text_color());

        // ...and an explicit colour wins over it.
        menu.set_hover_color(Color::rgb(255, 0, 0));
        assert_eq!(menu.hover_color(), Color::rgb(255, 0, 0));

        menu.set_text_color(Color::rgb(255, 255, 255));
        assert_eq!(menu.text_color(), Color::rgb(255, 255, 255));
    }

    /// 20. Animation progress
    #[test]
    fn test_animation_progress() {
        let mut menu = PieMenu::new(Point::new(100, 100), 80.0);

        // Default: the resting end, not the target end (BLUE24 §2.4 gate B).
        assert!((menu.animation_progress() - 0.0).abs() < 0.001);

        // Set value
        menu.set_animation_progress(0.5);
        assert!((menu.animation_progress() - 0.5).abs() < 0.001);

        // Clamped to [0.0, 1.0]
        menu.set_animation_progress(-0.5);
        assert!((menu.animation_progress() - 0.0).abs() < 0.001);

        menu.set_animation_progress(1.5);
        assert!((menu.animation_progress() - 1.0).abs() < 0.001);
    }
    /// The documented inner-radius band holds for every constructor input.
    ///
    /// `set_inner_radius` was `inner_radius.max(2.0).min(self.radius * 0.95)`: the floor was
    /// raised first and the ceiling applied second, so the ceiling won and the documented
    /// `2.0` lower bound was not actually guaranteed once `0.95 * radius < 2.0`.
    /// `set_radius` could not produce such a radius (it floors at 10.0) but `new` could,
    /// because it accepted any value — `new(center, 2.0)` then `set_inner_radius(1.0)` stored
    /// `1.9`. Both writers now floor the radius, and the clamp runs ceiling-then-floor.
    #[test]
    fn pie_menu_inner_radius_stays_inside_the_documented_band() {
        for radius in [100.0f32, 10.0, 5.0, 2.1, 2.0, 1.0, 0.0, -5.0] {
            let mut menu = PieMenu::new(Point::new(0, 0), radius);
            let outer = menu.radius();
            assert!(
                outer >= PieMenu::MIN_RADIUS,
                "new({radius}) produced radius {outer}, below the floor"
            );

            menu.set_inner_radius(1.0);
            assert!(
                menu.inner_radius() >= 2.0,
                "new({radius}): inner {} is below the documented floor 2.0",
                menu.inner_radius()
            );

            menu.set_inner_radius(f32::MAX);
            assert!(
                menu.inner_radius() <= 0.95 * outer,
                "new({radius}): inner {} exceeds 0.95 * radius ({})",
                menu.inner_radius(),
                0.95 * outer
            );
        }
    }

    /// `new` floors the radius, exactly as `set_radius` does.
    ///
    /// A zero radius produced a zero-sized geometry, which makes every item unreachable: the
    /// hit test compares a distance against `inner_radius`/`radius`, and there is no point in a
    /// zero-radius menu that is inside it.
    #[test]
    fn pie_menu_new_floors_the_radius() {
        for radius in [0.0f32, -1.0, 0.5, 9.9] {
            let menu = PieMenu::new(Point::new(10, 10), radius);
            assert_eq!(menu.radius(), PieMenu::MIN_RADIUS, "new({radius}) must floor");
            assert_eq!(menu.geometry().width, (PieMenu::MIN_RADIUS * 2.0) as u32);
        }
        assert_eq!(PieMenu::new(Point::new(0, 0), 42.0).radius(), 42.0, "above the floor is kept");
    }

    /// A non-finite radius falls back to the floor rather than propagating.
    #[test]
    fn pie_menu_non_finite_radius_falls_back_to_the_floor() {
        let menu = PieMenu::new(Point::new(0, 0), f32::NAN);
        assert_eq!(menu.radius(), PieMenu::MIN_RADIUS);

        let mut menu = PieMenu::new(Point::new(0, 0), 50.0);
        menu.set_radius(f32::INFINITY);
        assert_eq!(menu.radius(), PieMenu::MIN_RADIUS);
    }

    /// A non-finite inner radius is refused, leaving the previous value in place.
    ///
    /// `f32::max`/`min` return the non-NaN operand, so a NaN input used to silently become
    /// whichever bound the arithmetic happened to keep — a value the caller never asked for.
    #[test]
    fn pie_menu_non_finite_inner_radius_is_refused() {
        let mut menu = PieMenu::new(Point::new(0, 0), 100.0);
        menu.set_inner_radius(30.0);
        menu.set_inner_radius(f32::NAN);
        assert_eq!(menu.inner_radius(), 30.0, "NaN must not change the value");
        menu.set_inner_radius(f32::INFINITY);
        assert_eq!(menu.inner_radius(), 30.0, "infinity must not change the value");
    }

    /// Exhaustive proof that the `2.0 ..= 0.95 * radius` band
    /// [`PieMenu::set_inner_radius`] documents is actually unbreakable.
    ///
    /// The earlier order (`max(2.0)` then `min(0.95 * radius)`) made the ceiling win,
    /// so the floor was not really guaranteed whenever the band was unsatisfiable —
    /// which needed `radius < ~2.11`. `new` was the only way in, and it now floors the
    /// radius at [`PieMenu::MIN_RADIUS`]. This test pins the property rather than
    /// restating the reasoning: for every radius that can reach either writer,
    /// including the absurd ones, both bounds hold after either extreme inner radius.
    #[test]
    fn pie_menu_inner_radius_band_is_unbreakable() {
        for &radius in &[
            -1e9_f32,
            0.0,
            1.0,
            2.0,
            2.105,
            5.0,
            PieMenu::MIN_RADIUS,
            1e6,
            f32::INFINITY,
            f32::NAN,
        ] {
            let mut menu = PieMenu::new(Point::new(100, 100), radius);
            assert!(
                menu.radius() >= PieMenu::MIN_RADIUS,
                "new({radius}) left radius below MIN_RADIUS: {}",
                menu.radius()
            );

            menu.set_inner_radius(-5.0);
            let low = menu.inner_radius();
            menu.set_inner_radius(1e9);
            let high = menu.inner_radius();
            let ceiling = menu.radius() * 0.95;

            assert!(low >= 2.0 - 1e-4, "inner radius {low} fell below the 2.0 floor");
            assert!(
                high <= ceiling + 1e-4,
                "inner radius {high} rose above 0.95 * radius ({ceiling})"
            );
            assert!(low <= high, "the setters must be monotonic in the requested value");
        }
    }

    /// `set_radius` is the other writer of `radius`; shrinking it must pull the
    /// inner radius down with it rather than leaving it outside the band.
    #[test]
    fn pie_menu_shrinking_radius_pulls_inner_radius_down() {
        let mut menu = PieMenu::new(Point::new(0, 0), 200.0);
        menu.set_inner_radius(180.0);
        assert!(menu.inner_radius() <= menu.radius() * 0.95 + 1e-4);

        menu.set_radius(PieMenu::MIN_RADIUS);
        assert!(
            menu.inner_radius() <= menu.radius() * 0.95 + 1e-4,
            "inner radius {} escaped the band after shrinking to {}",
            menu.inner_radius(),
            menu.radius()
        );
        assert!(menu.inner_radius() >= 2.0 - 1e-4);
    }

    /// `set_inner_radius` asks for a frame.
    ///
    /// Like its siblings `set_radius`/`set_center`, it goes through `update_geometry`, which only
    /// stores the rectangle — so the caller owes the repaint.
    #[test]
    fn setting_inner_radius_requests_a_redraw() {
        let mut pm = PieMenu::new(crate::core::Point::new(100, 100), 80.0);
        let fired = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        {
            let f = std::sync::Arc::clone(&fired);
            let scope = pm.connection_scope();
            pm.redraw_requested_signal().connect_scoped(scope, move || {
                f.store(true, std::sync::atomic::Ordering::SeqCst);
            });
        }

        pm.set_inner_radius(12.0);
        assert!(fired.load(std::sync::atomic::Ordering::SeqCst), "a new hole radius must be drawn");
    }

    #[test]
    fn the_colour_setters_repaint() {
        // Both colours are read by `draw` — the highlight fill and the hovered label's contrast
        // colour — so a write that did not ask for a frame was invisible until something else
        // forced one.
        let mut menu = PieMenu::new(crate::core::Point::new(100, 100), 80.0);

        let seen = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        menu.base.redraw_requested.connect({
            let seen = std::sync::Arc::clone(&seen);
            move || {
                seen.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }
        });

        menu.set_hover_color(Color::rgb(10, 20, 30));
        assert_eq!(
            seen.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "a new hover colour owes a frame"
        );
        menu.set_hover_color(Color::rgb(10, 20, 30));
        assert_eq!(
            seen.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "and re-setting the same value does not"
        );

        menu.set_text_color(Color::rgb(40, 50, 60));
        assert_eq!(
            seen.load(std::sync::atomic::Ordering::SeqCst),
            2,
            "a new text colour owes a frame"
        );
    }

    /// An unset highlight and unset label ink follow the appearance — the light and dark
    /// derivations differ — while an explicit colour wins under either.
    ///
    /// The wedges' default fill is the themed ring's step, so the *rendered* light/dark
    /// difference of the whole menu is asserted one level up, by
    /// `p3_chrome_follows_the_appearance_unless_exempted_as_data` (the control is
    /// deliberately absent from the data-colour exemption table).
    #[test]
    fn the_unset_colours_follow_the_appearance() {
        let _guard = crate::style::theme_test_guard();
        crate::widget::census::install_preset_appearances();

        let resolved = || {
            let menu = PieMenu::new(Point::new(100, 100), 80.0);
            (menu.hover_color(), menu.text_color())
        };

        crate::theme::global_theme_manager().set_appearance(crate::theme::AppearanceMode::Light);
        let (light_hover, light_text) = resolved();
        crate::theme::global_theme_manager().set_appearance(crate::theme::AppearanceMode::Dark);
        let (dark_hover, dark_text) = resolved();

        assert_ne!(light_hover, dark_hover, "an unset highlight is chrome and must move");
        assert_ne!(light_text, dark_text, "unset label ink is chrome and must move");

        // An explicit colour overrides the theme under either appearance.
        let mut pinned = PieMenu::new(Point::new(100, 100), 80.0);
        pinned.set_hover_color(Color::rgb(255, 0, 0));
        pinned.set_text_color(Color::rgb(255, 255, 255));
        for mode in [crate::theme::AppearanceMode::Light, crate::theme::AppearanceMode::Dark] {
            crate::theme::global_theme_manager().set_appearance(mode);
            assert_eq!(pinned.hover_color(), Color::rgb(255, 0, 0), "under {mode:?}");
            assert_eq!(pinned.text_color(), Color::rgb(255, 255, 255), "under {mode:?}");
        }
    }

    /// The keyboard walks the ring and commits the highlighted slot.
    #[test]
    fn the_keyboard_walks_the_ring_and_commits() {
        use crate::event::EventHandler;

        let mut menu = PieMenu::new(Point::new(100, 100), 80.0);
        menu.add_item("Open");
        menu.add_item("Close");
        menu.add_item("Quit");
        menu.show_at(Point::new(100, 100));

        let chosen = Arc::new(Mutex::new(Vec::new()));
        let sink = chosen.clone();
        menu.triggered_text.connect(move |text| {
            sink.lock().unwrap().push(text.to_string());
        });

        menu.handle_event(&Event::key_press(crate::event::key_codes::RIGHT, 0));
        assert_eq!(menu.hovered_index(), Some(0));
        menu.handle_event(&Event::key_press(crate::event::key_codes::RIGHT, 0));
        assert_eq!(menu.hovered_index(), Some(1));
        menu.handle_event(&Event::key_press(crate::event::key_codes::LEFT, 0));
        assert_eq!(menu.hovered_index(), Some(0));
        menu.handle_event(&Event::key_press(crate::event::key_codes::ENTER, 0));
        assert_eq!(&*chosen.lock().unwrap(), &["Open".to_string()]);
        assert!(!menu.is_visible(), "committing a leaf closes the menu");
    }

    /// A branch opens a second ring, and Escape returns to the first.
    #[test]
    fn a_branch_opens_a_child_ring() {
        use crate::event::EventHandler;

        let mut menu = PieMenu::new(Point::new(100, 100), 80.0);
        let branch = PieMenuItem::new("Edit")
            .with_children(vec![PieMenuItem::new("Copy"), PieMenuItem::new("Paste")]);
        menu.add_existing_item(branch);
        menu.add_item("View");
        menu.show_at(Point::new(100, 100));

        assert!(menu.open_submenu(0));
        assert_eq!(menu.open_submenu_index(), Some(0));
        // The child ring is divided evenly among the two children.
        assert_eq!(menu.items()[0].children()[0].angle_start(), 0.0);
        assert_eq!(menu.items()[0].children()[1].angle_start(), std::f32::consts::PI);

        menu.handle_event(&Event::key_press(crate::event::key_codes::ESCAPE, 0));
        assert_eq!(menu.open_submenu_index(), None, "Escape returns one ring");
        assert!(menu.is_visible(), "and leaves the menu up");
        menu.handle_event(&Event::key_press(crate::event::key_codes::ESCAPE, 0));
        assert!(!menu.is_visible());
    }

    /// Separators are gaps: they are skipped by the keyboard and never hovered.
    #[test]
    fn separator_slots_are_skipped() {
        let mut menu = PieMenu::new(Point::new(100, 100), 80.0);
        menu.add_item("Before");
        let mut gap = PieMenuItem::new("");
        gap.set_separator(true);
        menu.add_existing_item(gap);
        menu.add_item("After");
        menu.show_at(Point::new(100, 100));

        menu.handle_event(&Event::key_press(crate::event::key_codes::RIGHT, 0));
        assert_eq!(menu.hovered_index(), Some(0));
        menu.handle_event(&Event::key_press(crate::event::key_codes::RIGHT, 0));
        assert_eq!(menu.hovered_index(), Some(2), "the gap is stepped over");
        assert!(menu.items()[1].is_separator());
    }

    /// A shortcut label is painted, and the ring reveal is driven by `tick`.
    #[test]
    fn shortcut_labels_and_the_reveal_reach_the_painter() {
        use crate::widget::Widget;

        let mut menu = PieMenu::new(Point::new(80, 80), 70.0);
        menu.add_existing_item(PieMenuItem::new("Copy").with_shortcut("Ctrl+C"));
        menu.add_existing_item(PieMenuItem::new("Paste"));
        menu.show_at(Point::new(80, 80));
        menu.set_animation_progress(1.0);
        let without = crate::widget::svg::render_to_svg(&mut menu);

        menu.items_mut()[0].set_shortcut(Some("Ctrl+Shift+C".to_string()));
        let with = crate::widget::svg::render_to_svg(&mut menu);
        assert_ne!(without, with, "the shortcut hint must reach the painter");

        menu.set_animation_progress(0.0);
        assert_eq!(menu.animation_progress(), 0.0);
        menu.set_animation_progress(1.0);
        assert!(!menu.is_animating());
        assert!((menu.animation_progress() - 1.0).abs() < f32::EPSILON);
    }

    /// A branch inside an open branch opens another ring instead of reporting a choice (ADV-08),
    /// and every activation carries the item's id (ADV-09).
    #[test]
    fn submenus_nest_and_activation_carries_the_id() {
        let mut menu = PieMenu::new(Point::new(80, 80), 70.0);
        menu.show_at(Point::new(80, 80));
        menu.set_animation_progress(1.0);
        let branch = menu.add_existing_item(
            PieMenuItem::new("File").with_children(vec![PieMenuItem::new("Recent")]),
        );
        assert_ne!(branch, 0);

        let ids = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&ids);
        menu.triggered_id.connect(move |id| sink.lock().unwrap().push(*id));

        assert!(menu.open_submenu(0));
        assert_eq!(menu.submenu_depth(), 1);
        menu.handle_event(&Event::key_press(crate::event::key_codes::ESCAPE, 0));
        assert_eq!(menu.submenu_depth(), 0, "Escape unwinds one ring");

        // A leaf activation reports its id.
        menu.set_current_index(0);
        assert_eq!(ids.lock().unwrap().len(), 1);
        assert_eq!(ids.lock().unwrap()[0], branch, "the payload is the item's id, not its index");
    }

    /// A branch inside an open branch opens another ring instead of reporting a choice (ADV-08).
    #[test]
    fn a_nested_branch_opens_a_second_ring() {
        let mut menu = PieMenu::new(Point::new(80, 80), 70.0);
        menu.show_at(Point::new(80, 80));
        menu.set_animation_progress(1.0);
        menu.add_existing_item(PieMenuItem::new("File").with_children(vec![
            PieMenuItem::new("Recent").with_children(vec![PieMenuItem::new("notes.txt")]),
        ]));
        assert!(menu.open_submenu(0));
        assert_eq!(menu.submenu_depth(), 1);
        // "Recent" is itself a branch: the old single-slot form refused this, so a grandchild
        // branch could never be entered.
        assert!(menu.open_submenu(0), "a branch inside an open branch opens another ring");
        assert_eq!(menu.submenu_depth(), 2);
        menu.handle_event(&Event::key_press(crate::event::key_codes::ESCAPE, 0));
        assert_eq!(menu.submenu_depth(), 1, "Escape unwinds one ring at a time");
        menu.close_submenu();
        assert_eq!(menu.submenu_depth(), 0);
    }

    /// `items` round-trips through the record property, including the fields the control owns.
    ///
    /// This is the payload channel `add_item`/`remove_item` never had (B26.03): a document spells
    /// the whole ring, and the control's own `add_existing_item` recomputes the angles and advances
    /// the id counter past a pinned id.
    #[test]
    fn items_round_trip_through_the_property() {
        use crate::widget::capability::{RecordList, RecordValue, EMPTY_RECORD_SCHEMA};

        let mut menu = PieMenu::new(Point::new(80, 80), 70.0);
        let list = RecordList::from_items(
            &EMPTY_RECORD_SCHEMA,
            vec![
                RecordValue::new()
                    .with("id", CapabilityValue::UInt(7))
                    .with("text", CapabilityValue::String(String::from("Copy")))
                    .with("shortcut", CapabilityValue::String(String::from("Ctrl+C"))),
                RecordValue::new()
                    .with("id", CapabilityValue::UInt(8))
                    .with("text", CapabilityValue::String(String::from("Paste")))
                    .with("enabled", CapabilityValue::Bool(false)),
                RecordValue::new()
                    .with("id", CapabilityValue::UInt(9))
                    .with("text", CapabilityValue::String(String::new()))
                    .with("separator", CapabilityValue::Bool(true)),
            ],
        );
        menu.set("items", CapabilityValue::Records(list)).expect("a well-formed ring is accepted");

        assert_eq!(menu.item_count(), 3);
        assert_eq!(menu.items()[0].id(), 7);
        assert_eq!(menu.items()[0].shortcut(), Some("Ctrl+C"));
        assert!(!menu.items()[1].is_enabled());
        assert!(menu.items()[2].is_separator());
        // The write goes through the control's own normalisation, so the ring is evenly divided.
        assert!(menu.items()[0].angle_end() > menu.items()[0].angle_start());
        // A pinned id pushes the auto-assignment counter past it.
        assert_ne!(menu.add_item("Later"), 7);
        assert_ne!(menu.add_item("Later still"), 9);

        let CapabilityValue::Records(read_back) = menu.get("items").expect("the ring reads back")
        else {
            panic!("`items` must read as a record list");
        };
        assert_eq!(read_back.len(), 5);
        assert!(matches!(
            read_back.items()[0].get("shortcut"),
            Some(CapabilityValue::String(shortcut)) if shortcut == "Ctrl+C"
        ));
        assert!(matches!(read_back.items()[2].get("separator"), Some(CapabilityValue::Bool(true))));
    }

    /// A malformed or duplicate-id list is refused **and the live ring is untouched**.
    #[test]
    fn a_malformed_item_list_is_refused_and_changes_nothing() {
        use crate::widget::capability::{RecordList, RecordValue, EMPTY_RECORD_SCHEMA};

        let mut menu = PieMenu::new(Point::new(80, 80), 70.0);
        menu.add_item("Existing");
        let before: Vec<String> = menu.items().iter().map(|item| item.text().to_string()).collect();

        let broken = vec![
            ("missing text", vec![RecordValue::new().with("id", CapabilityValue::UInt(1))]),
            (
                "unknown field",
                vec![RecordValue::new()
                    .with("id", CapabilityValue::UInt(1))
                    .with("text", CapabilityValue::String(String::from("A")))
                    .with("weight", CapabilityValue::UInt(1))],
            ),
            (
                "duplicate id",
                vec![
                    RecordValue::new()
                        .with("id", CapabilityValue::UInt(1))
                        .with("text", CapabilityValue::String(String::from("A"))),
                    RecordValue::new()
                        .with("id", CapabilityValue::UInt(1))
                        .with("text", CapabilityValue::String(String::from("B"))),
                ],
            ),
        ];

        for (reason, records) in broken {
            let list = RecordList::from_items(&EMPTY_RECORD_SCHEMA, records);
            let result = menu.set("items", CapabilityValue::Records(list));
            assert!(
                matches!(result, Err(CapabilityAccessError::ConstraintViolation { .. })),
                "{reason} must be refused with a pointable constraint violation (got {result:?})"
            );
            let after: Vec<String> =
                menu.items().iter().map(|item| item.text().to_string()).collect();
            assert_eq!(after, before, "{reason} must change nothing");
        }
    }

    /// `items` is published as a writable record property with its schema.
    #[test]
    fn the_items_property_is_published_with_its_record_schema() {
        let menu = PieMenu::new(Point::new(80, 80), 70.0);
        assert!(WidgetProperties::property_names(&menu).contains(&"items"));
        let schema = crate::widget::capability::PIE_MENU_PROPERTIES
            .iter()
            .find(|schema| schema.name == "items")
            .expect("`items` is declared by the capability table");
        assert_eq!(schema.value_kind, crate::widget::capability::PropertyValueKind::Records);
        assert!(schema.readable && schema.writable);
        let record_schema = schema.record_schema.expect("a `Records` schema names its shape");
        assert_eq!(record_schema.id_field, "id");
        assert_eq!(
            record_schema.names(),
            vec!["id", "text", "icon", "icon_source", "enabled", "shortcut", "separator"]
        );
        // The identity field is a `UInt`; the list-level unique/required rules read text only, so
        // the control enforces uniqueness itself and the schema does not claim otherwise.
        assert!(!record_schema.ids_unique && !record_schema.ids_required);
        assert_eq!(record_schema, &PIE_MENU_ITEM_SCHEMA);
    }

    /// A static ring reuses its wedge contours; hover and a resize are handled as stated.
    #[test]
    fn an_idle_menu_reuses_its_slice_paths() {
        let mut menu = PieMenu::new(Point::new(80, 80), 70.0);
        menu.add_item("Cut");
        menu.add_item("Copy");
        menu.add_item("Paste");
        menu.show_at(Point::new(80, 80));
        menu.set_animation_progress(1.0);

        let _ = render_to_svg(&mut menu);
        let key = menu.slice_paths.borrow().key.expect("the first frame built the paths");
        let lengths: Vec<usize> =
            menu.slice_paths.borrow().slices.iter().map(|path| path.points.len()).collect();
        assert_eq!(lengths.len(), 3, "one cached contour per slot");
        assert!(lengths.iter().all(|len| *len > 0), "every wedge encloses an area");

        let _ = render_to_svg(&mut menu);
        assert_eq!(menu.slice_paths.borrow().key, Some(key), "an idle frame reuses the paths");

        // Hover is a fill colour, not geometry, so it must not rebuild the contours.
        menu.hovered_index = Some(1);
        let _ = render_to_svg(&mut menu);
        assert_eq!(menu.slice_paths.borrow().key, Some(key), "hover does not resample");

        // A radius change is geometry, so it must rebuild.
        menu.set_radius(60.0);
        let _ = render_to_svg(&mut menu);
        assert_ne!(menu.slice_paths.borrow().key, Some(key), "a resize rebuilds the paths");
    }
}
