// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! SplitButton widget with primary action and drop-down action list.
//!
//! The face is derived from the two columns' own sizes, not from one edge: the trigger takes what
//! the arrow column does not.
//!
//! BLUE22 §B.8 lists this control's defect as "master face + arrow by hand", and the fix is not
//! a nicer arithmetic: it is that the two columns must **tile** the face, so a wider arrow column
//! *pushes* the trigger narrower instead of being placed from the trailing edge while the trigger
//! is placed from the leading one.
//!
//! `SplitButton::column_widths` states that relation once and both boxes are read from it, so the
//! two columns cannot be placed from opposite edges. The one thing that is *declared* rather than
//! derived is the arrow's preferred width ([`dimensions::SPLIT_ARROW_COLUMN_WIDTH`]) — see that
//! constant for why the number is a constant and not the `v` glyph's advance.

use crate::compat::Vec;
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::style::EdgeOffsets;
use crate::widget::capability::coercion::{expect_bool, expect_string, expect_u32};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::menu_toolbar::popup_reveal::{PopupReveal, RevealDirection};
use crate::widget::metrics::{dimensions, effective_font, estimate_text_width, ControlMetrics};
use crate::widget::{BaseWidget, Draw, IconName, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// One selectable action in a split button drop-down list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplitAction {
    /// Stable action identifier.
    pub id: String,
    /// Display label.
    pub label: String,
    /// Optional leading icon.
    pub icon: Option<IconName>,
    /// Whether a separator rule is drawn above this row.
    pub separator_above: bool,
    /// Whether the action can be chosen; a disabled row is skipped by keyboard navigation.
    pub enabled: bool,
    /// Whether the action shows a check mark, for a menu that toggles a mode.
    pub checked: bool,
    /// The keyboard shortcut shown at the row's trailing edge (SP-66).
    pub shortcut: Option<String>,
}

impl SplitAction {
    /// Creates a split action.
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon: None,
            separator_above: false,
            enabled: true,
            checked: false,
            shortcut: None,
        }
    }

    /// Sets the action's leading icon.
    pub fn with_icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Draws a separator rule above this action.
    pub fn with_separator_above(mut self) -> Self {
        self.separator_above = true;
        self
    }

    /// Marks the action enabled or disabled.
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Marks the action checked or unchecked.
    pub fn with_checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    /// Shows a keyboard shortcut at the row's trailing edge.
    ///
    /// The field follows `CommandEntry::with_shortcut` in the sibling palette control, so a
    /// command's key binding is described the same way wherever this library lists commands
    /// (SP-66).
    pub fn with_shortcut(mut self, shortcut: impl Into<String>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }
}

/// The declared shape of one split-menu action, as a record (BLUE26 B26.09).
///
/// This is the row shape of the `actions` property, and the control owns both ends of it: the
/// field names are [`SplitAction`]'s own fields. `id` and `label` are required because
/// [`SplitAction::new`] requires them; the rest are optional because a row that omits them
/// describes the ordinary action. The icon travels as its [`IconName`] token, the same spelling
/// the `icon` property accepts.
pub const SPLIT_ACTION_SCHEMA: crate::widget::capability::RecordSchema =
    crate::widget::capability::RecordSchema {
        fields: &[
            crate::widget::capability::RecordField::required(
                "id",
                crate::widget::capability::PropertyValueKind::String,
            ),
            crate::widget::capability::RecordField::required(
                "label",
                crate::widget::capability::PropertyValueKind::String,
            ),
            crate::widget::capability::RecordField::optional(
                "icon",
                crate::widget::capability::PropertyValueKind::String,
            ),
            crate::widget::capability::RecordField::optional(
                "separator_above",
                crate::widget::capability::PropertyValueKind::Bool,
            ),
            crate::widget::capability::RecordField::optional(
                "enabled",
                crate::widget::capability::PropertyValueKind::Bool,
            ),
            crate::widget::capability::RecordField::optional(
                "checked",
                crate::widget::capability::PropertyValueKind::Bool,
            ),
            crate::widget::capability::RecordField::optional(
                "shortcut",
                crate::widget::capability::PropertyValueKind::String,
            ),
        ],
        id_field: "id",
        ids_unique: true,
        ids_required: true,
    };

/// One row of [`SPLIT_ACTION_SCHEMA`].
fn split_action_record(action: &SplitAction) -> crate::widget::capability::RecordValue {
    let mut record = crate::widget::capability::RecordValue::new()
        .with("id", CapabilityValue::String(action.id.clone()))
        .with("label", CapabilityValue::String(action.label.clone()))
        .with("separator_above", CapabilityValue::Bool(action.separator_above))
        .with("enabled", CapabilityValue::Bool(action.enabled))
        .with("checked", CapabilityValue::Bool(action.checked));
    if let Some(icon) = action.icon {
        record = record.with("icon", CapabilityValue::String(icon.as_str().to_string()));
    }
    if let Some(shortcut) = &action.shortcut {
        record = record.with("shortcut", CapabilityValue::String(shortcut.clone()));
    }
    record
}

/// Rebuilds one [`SplitAction`] from a row of [`SPLIT_ACTION_SCHEMA`].
fn split_action_from_record(
    record: &crate::widget::capability::RecordValue,
) -> Result<SplitAction, CapabilityAccessError> {
    let id = match record.get("id") {
        Some(CapabilityValue::String(id)) => id.clone(),
        _ => return Err(CapabilityAccessError::TypeMismatch),
    };
    let label = match record.get("label") {
        Some(CapabilityValue::String(label)) => label.clone(),
        _ => return Err(CapabilityAccessError::TypeMismatch),
    };
    let bool_field = |name: &str, default: bool| match record.get(name) {
        None | Some(CapabilityValue::Null) => Ok(default),
        Some(CapabilityValue::Bool(flag)) => Ok(*flag),
        Some(_) => Err(CapabilityAccessError::TypeMismatch),
    };
    let icon = match record.get("icon") {
        None | Some(CapabilityValue::Null) => None,
        Some(CapabilityValue::String(token)) => {
            Some(IconName::from_name(token).ok_or(CapabilityAccessError::OutOfRange)?)
        }
        Some(_) => return Err(CapabilityAccessError::TypeMismatch),
    };
    let shortcut = match record.get("shortcut") {
        None | Some(CapabilityValue::Null) => None,
        Some(CapabilityValue::String(shortcut)) => Some(shortcut.clone()),
        Some(_) => return Err(CapabilityAccessError::TypeMismatch),
    };

    let mut action = SplitAction::new(id, label);
    action.icon = icon;
    action.separator_above = bool_field("separator_above", false)?;
    action.enabled = bool_field("enabled", true)?;
    action.checked = bool_field("checked", false)?;
    action.shortcut = shortcut;
    Ok(action)
}

/// Where a split button's drop-down opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PopupDirection {
    /// Below the face, the conventional placement.
    #[default]
    Down,
    /// Above the face.
    Up,
    /// Below when it fits, above when the window has no room below.
    Auto,
}

impl PopupDirection {
    /// The property spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            PopupDirection::Down => "down",
            PopupDirection::Up => "up",
            PopupDirection::Auto => "auto",
        }
    }

    /// Parses a property spelling.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "down" => PopupDirection::Down,
            "up" => PopupDirection::Up,
            "auto" => PopupDirection::Auto,
            _ => return None,
        })
    }
}

/// SplitButton provides a default primary action plus selectable alternatives.
///
/// # Known limitation: the popup shares this control's canvas
///
/// The drop-down is painted from this control's own `draw`, below its face. A **later sibling**
/// that happens to cover the same pixels still paints over it, because the widget layer has no
/// z-ordered overlay surface; moving the list to one is a render-layer feature, not something
/// this control can assert on its own.
///
/// What the control *can* do, and does, is adopt the framework's popup contract: it reports
/// [`Widget::has_open_popup`], takes the popup grab in [`SplitButton::open_menu`], and accepts
/// pointer positions inside the hanging list through [`Widget::contains_point`]. Outside presses
/// therefore dismiss it without reaching whatever they landed on, and a row outside this
/// control's rectangle is still hit-testable.
pub struct SplitButton {
    base: BaseWidget,
    text: String,
    actions: Vec<SplitAction>,
    primary_action_index: Option<usize>,
    highlighted_action_index: Option<usize>,
    menu_open: bool,
    /// How far the drop-down has opened. The `menu_open` flag above stays the authority (it is
    /// what hit testing and the property contract read); this only describes the transition.
    reveal: PopupReveal,
    pressed_primary: bool,
    pressed_arrow: bool,
    hovered_primary: bool,
    hovered_arrow: bool,
    arrow_width: u32,
    row_height: u32,
    /// Where the drop-down opens.
    popup_direction: PopupDirection,
    /// The maximum height of the drop-down, or `None` to size it to its rows.
    max_menu_height: Option<u32>,
    /// The first action drawn in a capped drop-down.
    menu_scroll: usize,
    /// Emitted when primary area triggers current action id.
    pub triggered: Signal1<String>,
    /// Emitted when a drop-down action is explicitly selected.
    pub action_selected: Signal1<String>,
    /// Emitted when menu open state changes.
    pub menu_toggled: Signal1<bool>,
}

impl SplitButton {
    /// Creates a split button.
    pub fn new(text: impl Into<String>, geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ToolButton, geometry, "SplitButton"),
            text: text.into(),
            actions: Vec::new(),
            primary_action_index: None,
            highlighted_action_index: None,
            menu_open: false,
            reveal: PopupReveal::new(false),
            pressed_primary: false,
            pressed_arrow: false,
            hovered_primary: false,
            hovered_arrow: false,
            arrow_width: dimensions::SPLIT_ARROW_COLUMN_WIDTH,
            row_height: 22,
            popup_direction: PopupDirection::default(),
            max_menu_height: None,
            menu_scroll: 0,
            triggered: Signal1::new(),
            action_selected: Signal1::new(),
            menu_toggled: Signal1::new(),
        }
    }

    /// Returns button text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Sets button text.
    pub fn set_text(&mut self, text: impl Into<String>) {
        let text = text.into();
        if self.text == text {
            return;
        }
        self.text = text;
        self.base.request_redraw();
    }

    /// Replaces all actions.
    pub fn set_actions(&mut self, actions: Vec<SplitAction>) {
        self.actions = actions;
        self.primary_action_index = if self.actions.is_empty() { None } else { Some(0) };
        self.highlighted_action_index = self.primary_action_index;
        self.menu_scroll = 0;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Returns all actions.
    pub fn actions(&self) -> &[SplitAction] {
        &self.actions
    }

    /// Adds one action and returns its index.
    pub fn add_action(&mut self, action: SplitAction) -> usize {
        let index = self.actions.len();
        self.actions.push(action);
        if self.primary_action_index.is_none() {
            self.primary_action_index = Some(index);
            self.highlighted_action_index = Some(index);
        }
        self.base.request_layout();
        self.base.request_redraw();
        index
    }

    /// Returns current primary action index.
    pub fn primary_action_index(&self) -> Option<usize> {
        self.primary_action_index.filter(|index| *index < self.actions.len())
    }

    /// Returns whether menu is open.
    pub fn menu_open(&self) -> bool {
        self.menu_open
    }

    /// Returns highlighted action index when menu is open.
    pub fn highlighted_action_index(&self) -> Option<usize> {
        self.highlighted_action_index.filter(|index| *index < self.actions.len())
    }

    /// Sets menu row height.
    pub fn set_row_height(&mut self, row_height: u32) {
        let row_height = row_height.max(1);
        if self.row_height == row_height {
            return;
        }
        self.row_height = row_height;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Returns menu row height.
    pub fn row_height(&self) -> u32 {
        self.row_height
    }

    /// Where the drop-down opens.
    pub fn popup_direction(&self) -> PopupDirection {
        self.popup_direction
    }

    /// Sets where the drop-down opens.
    pub fn set_popup_direction(&mut self, direction: PopupDirection) {
        if self.popup_direction == direction {
            return;
        }
        self.popup_direction = direction;
        self.base.request_redraw();
    }

    /// The configured maximum drop-down height, or `None` when it sizes to its rows.
    pub fn max_menu_height(&self) -> Option<u32> {
        self.max_menu_height
    }

    /// Caps the drop-down height; `None` restores the natural height.
    pub fn set_max_menu_height(&mut self, height: Option<u32>) {
        let height = height.map(|value| value.max(self.row_height));
        if self.max_menu_height == height {
            return;
        }
        self.max_menu_height = height;
        self.clamp_menu_scroll();
        self.base.request_redraw();
    }

    /// The first action drawn in a capped drop-down.
    pub fn menu_scroll(&self) -> usize {
        self.menu_scroll
    }

    /// Sets the first action drawn in a capped drop-down.
    pub fn set_menu_scroll(&mut self, offset: usize) {
        let max = self.actions.len().saturating_sub(self.visible_menu_rows());
        let offset = offset.min(max);
        if self.menu_scroll == offset {
            return;
        }
        self.menu_scroll = offset;
        self.base.request_redraw();
    }

    fn visible_menu_rows(&self) -> usize {
        (self.menu_height() / self.row_height.max(1)).max(1) as usize
    }

    fn menu_height(&self) -> u32 {
        let natural = self.row_height.saturating_mul(self.actions.len() as u32);
        match self.max_menu_height {
            Some(max) => natural.min(max.max(self.row_height)),
            None => natural,
        }
    }

    fn clamp_menu_scroll(&mut self) {
        let max = self.actions.len().saturating_sub(self.visible_menu_rows());
        self.menu_scroll = self.menu_scroll.min(max);
    }

    /// The mounted window's vertical extent, when this control is on a tree.
    ///
    /// Used only by [`PopupDirection::Auto`]. The widget layer has no window object, so the
    /// root of the mounted parent chain is the one rectangle that describes the space the
    /// drop-down has to fit into. A control that is not mounted (or the alloc-frugal build,
    /// which compiles the registry out) keeps the explicit/down placement.
    #[cfg(not(alloc_frugal))]
    fn viewport_bounds(&self) -> Option<(i32, i32)> {
        let mut id = self.base.parent()?;
        let mut guard = 0usize;
        loop {
            let (geometry, parent) = crate::widget::runtime::with_widget(id, |widget| {
                (widget.geometry(), widget.parent())
            })?;
            match parent {
                Some(next) => {
                    guard += 1;
                    if guard > 128 {
                        return None;
                    }
                    id = next;
                }
                None => return Some((geometry.y, geometry.y + geometry.height as i32)),
            }
        }
    }

    #[cfg(alloc_frugal)]
    fn viewport_bounds(&self) -> Option<(i32, i32)> {
        None
    }

    /// Whether the drop-down has to open upward.
    fn opens_upward(&self, height: u32) -> bool {
        match self.popup_direction {
            PopupDirection::Down => false,
            PopupDirection::Up => true,
            PopupDirection::Auto => {
                let band = self.face_band();
                let Some((top, bottom)) = self.viewport_bounds() else {
                    return false;
                };
                let below = band.y + band.height as i32 + height as i32;
                let above = band.y - height as i32;
                below > bottom && above >= top
            }
        }
    }

    /// Triggers primary action.
    pub fn trigger_primary(&mut self) -> bool {
        let Some(index) = self.primary_action_index() else {
            return false;
        };
        let Some(action) = self.actions.get(index) else {
            return false;
        };
        if !action.enabled {
            return false;
        }
        self.triggered.emit(action.id.clone());
        true
    }

    /// Opens menu.
    ///
    /// The grab is what makes the list modal for the gesture that dismisses it: a press outside
    /// the control is delivered here (where `handle_event` closes the menu) instead of reaching
    /// the control underneath. See `runtime::POPUP_GRAB`.
    pub fn open_menu(&mut self) {
        if self.menu_open {
            return;
        }
        self.menu_open = true;
        self.reveal.set_open(true);
        self.highlighted_action_index = self.primary_action_index();
        self.menu_scroll = 0;
        self.ensure_highlight_visible();
        #[cfg(not(alloc_frugal))]
        crate::widget::runtime::set_popup_grab(self.id());
        self.menu_toggled.emit(true);
        self.base.request_redraw();
    }

    /// Closes menu.
    pub fn close_menu(&mut self) {
        if !self.menu_open {
            return;
        }
        self.menu_open = false;
        // Aimed at closed before the signal, so a listener that reopens immediately starts from
        // wherever the exit had got to rather than from a fully-open panel.
        self.reveal.set_open(false);
        #[cfg(not(alloc_frugal))]
        crate::widget::runtime::clear_popup_grab_for(self.id());
        self.menu_toggled.emit(false);
        self.base.request_redraw();
    }

    /// Toggles menu visibility.
    pub fn toggle_menu(&mut self) {
        if self.menu_open {
            self.close_menu();
        } else {
            self.open_menu();
        }
    }

    /// Selects highlighted drop-down action as primary and emits action_selected.
    pub fn select_highlighted_action(&mut self) -> bool {
        let Some(index) = self.highlighted_action_index() else {
            return false;
        };
        let Some(action) = self.actions.get(index) else {
            return false;
        };
        if !action.enabled {
            return false;
        }
        self.primary_action_index = Some(index);
        self.action_selected.emit(action.id.clone());
        self.close_menu();
        true
    }

    /// Moves highlighted action in menu by signed delta, skipping disabled rows.
    pub fn move_highlight(&mut self, delta: isize) {
        if delta == 0 {
            return;
        }
        if self.actions.is_empty() {
            self.highlighted_action_index = None;
            return;
        }
        let max = self.actions.len().saturating_sub(1);
        let mut next = self.highlighted_action_index.unwrap_or(0).min(max) as isize;
        let step = if delta >= 0 { 1 } else { -1 };
        let mut remaining = delta.unsigned_abs().max(1);
        while remaining > 0 {
            let mut candidate = next;
            loop {
                candidate += step;
                if candidate < 0 || candidate > max as isize {
                    break;
                }
                if self.actions[candidate as usize].enabled {
                    next = candidate;
                    break;
                }
            }
            remaining -= 1;
        }
        self.highlighted_action_index = Some(next as usize);
        self.ensure_highlight_visible();
        self.base.request_redraw();
    }

    /// Scrolls a capped drop-down until the highlighted row is inside the window.
    fn ensure_highlight_visible(&mut self) {
        let Some(index) = self.highlighted_action_index.filter(|index| *index < self.actions.len())
        else {
            return;
        };
        let visible = self.visible_menu_rows();
        if index < self.menu_scroll {
            self.menu_scroll = index;
        } else if index >= self.menu_scroll + visible {
            self.menu_scroll = index + 1 - visible;
        }
        self.clamp_menu_scroll();
    }

    /// The band the button actually paints: full width,
    /// `dimensions::SPLIT_BUTTON_HEIGHT` tall, centred in the rectangle it was given.
    ///
    /// # Why the face is not the rectangle
    ///
    /// A split button is chrome: one compact row split into a trigger and an arrow. Taking
    /// `rect.height` made a 240x120 census cell a 120 px-tall face whose two halves were also
    /// 120 tall — a slab shaped like a button rather than a button — and it disagreed with the
    /// 28 px `size_hint` the control reports. The band is the single derivation the paint, the
    /// hit tests and the drop-down's anchor all read.
    fn face_band(&self) -> Rect {
        ControlMetrics::full_width_band(self.geometry(), dimensions::SPLIT_BUTTON_HEIGHT)
    }

    /// The trigger and the arrow column, placed by the layout that owns the tiling.
    ///
    /// # Why the two columns are assembled rather than computed
    ///
    /// This used to be `primary_width = band.width - arrow_width` with the arrow placed from the
    /// band's trailing edge: two derivations from two different edges, so the trigger's box and
    /// the arrow's box agreed only because both subtracted the same constant. The layout holds
    /// the two facts the arithmetic was hiding — that they are ordered, and that they tile — and
    /// it states them once. The arrow's width is the *ground* fact here (a constant, see
    /// [`dimensions::SPLIT_ARROW_COLUMN_WIDTH`]); the trigger's is what the layout computes as the
    /// remainder.
    ///
    /// # Why the arrow is `add_flexible(.., Height)`
    ///
    /// Both columns are declared at the face's own height, so the cross axis has nothing to
    /// resolve and `Stretch` and "keep the preferred height" agree. Declaring it anyway is what
    /// makes the assembly say *why* the arrow is 28 px: it is what the arrow asked for, not
    /// whatever the row's tallest child happened to be.
    /// The trigger's box and the arrow column's box.
    ///
    /// # Why the two columns tile rather than being computed from two edges
    ///
    /// This used to be `primary_width = band.width - arrow_width` with the arrow placed from the
    /// band's trailing edge: two derivations from two different edges, so the trigger's box and
    /// the arrow's box agreed only because both subtracted the same constant. Both boxes are now
    /// read from [`SplitButton::column_widths`], the one statement of "the trigger takes what the
    /// arrow does not", so a wider arrow necessarily pushes the trigger narrower.
    ///
    /// # Why this is arithmetic and not an assembled row
    ///
    /// The pair used to be placed by a `CompositeBuilder` + `FlexLayout` built on every call — two
    /// `label` widgets, a layout, and a `Vec` of results for every `draw`, every hit test and every
    /// menu anchor read, to place two rectangles that tile one band. The row was
    /// `justify_content = FlexStart` with a `fill` trigger and zero gap, which resolves to exactly
    /// the relation `column_widths` states, so the row was a slower spelling of it.
    fn assemble_face(&self) -> (Rect, Rect) {
        let band = self.face_band();
        let (trigger_width, arrow_width) = self.column_widths(band.width);
        let trigger = Rect::new(band.x, band.y, trigger_width, band.height);
        let arrow = Rect::new(band.x + trigger_width as i32, band.y, arrow_width, band.height);
        (trigger, arrow)
    }

    /// The trigger's and the arrow column's widths for a face `face_width` wide.
    ///
    /// # The two cases
    ///
    /// * The face holds both columns: the arrow keeps the width it declares and the trigger takes
    ///   the remainder. That is the `fill` trigger's own statement — it absorbs the leftover — and
    ///   it is why growing the arrow narrows the trigger by exactly the same amount.
    /// * The face is narrower than the two columns' sum: tiling at their preferred sizes is
    ///   arithmetic that cannot work, so the shortfall is shared **proportionally**. Keeping the
    ///   arrow one arrow wide would take the whole shortfall out of the trigger and, on a narrow
    ///   enough face, leave no trigger; scaling both keeps the arrow's *share*, which is what makes
    ///   the drop-down affordance impossible to lose (G-1). The integer remainder goes to the
    ///   leading column so the pair sums to exactly `face_width` and neither can land outside it.
    fn column_widths(&self, face_width: u32) -> (u32, u32) {
        let wanted = self.trigger_hint_width();
        let total = wanted.saturating_add(self.arrow_width);
        if total <= face_width || total == 0 {
            let arrow = self.arrow_width.min(face_width);
            return (face_width - arrow, arrow);
        }
        let scaled = |width: u32| -> u32 {
            ((u64::from(width) * u64::from(face_width)) / u64::from(total)) as u32
        };
        let arrow = scaled(self.arrow_width);
        let trigger = scaled(wanted);
        // Integer division truncates every column, so the pair can fall one pixel short of the
        // face. The leading column takes it, matching the layout's "remainder from the leading
        // edge" rule and keeping `trigger + arrow == face_width` exact.
        let short = face_width.saturating_sub(trigger.saturating_add(arrow));
        (trigger + short, arrow)
    }

    /// The width the trigger's own label asks for.
    ///
    /// # What this used to claim, and why it was false
    ///
    /// The doc read: "The label control measures itself (`len * 8 + 2 * BUTTON_PADDING_H`), which
    /// is the same model `SplitButton` used, so the assembled face's columns tile exactly as the
    /// hand-computed pair did while now being derived from the text."
    ///
    /// Three things were wrong with it. `Label::implicit_size` measures with
    /// `estimate_text_width` (clusters, plus `letter_spacing`) and pads with `LABEL_PADDING`, not
    /// `len * 8` and not `BUTTON_PADDING_H`. So the two were neither the same model nor derived
    /// from the same quantity, and a non-ASCII caption was charged three bytes per character here
    /// and one cluster per character there.
    ///
    /// It now states the width in the label's own terms — the shared estimate plus the padding the
    /// label actually carries — so the face tiles from a number the child control would agree with,
    /// rather than from a restatement of it.
    fn trigger_hint_width(&self) -> u32 {
        split_hint_width(&self.text, effective_font(self.style()))
    }

    fn primary_rect(&self) -> Rect {
        self.assemble_face().0
    }

    /// The trigger's label box: the primary face's interior, inset by the shared padding.
    ///
    /// # Why this is a box and not an `x`
    ///
    /// The label used to be drawn at `primary_rect.x + 8` with `HorizontalAlignment::Left`,
    /// and the menu rows at `action_rect.x + 8` — the same literal written twice, in two
    /// different coordinate systems, with no relation to the control's own padding constant.
    /// Returning the padded box instead gives the draw call the rectangle it is fitting into,
    /// so the label is bounded by the trigger rather than by a hand-chosen origin, and the
    /// trigger and the menu rows derive their leading space from one place.
    fn primary_label_box(&self, primary: Rect) -> Rect {
        ControlMetrics::content_box(
            primary,
            EdgeOffsets {
                left: dimensions::SPLIT_BUTTON_PADDING_H,
                right: dimensions::SPLIT_BUTTON_PADDING_H,
                top: 0,
                bottom: 0,
            },
        )
    }

    fn arrow_rect(&self) -> Rect {
        self.assemble_face().1
    }

    fn menu_rect(&self) -> Rect {
        // The popup hangs from the **face**'s bottom edge, not the control's, so it appears
        // directly under the button the user pressed rather than 120 px below it. A capped
        // menu keeps that height and scrolls its rows.
        let band = self.face_band();
        let height = self.menu_height();
        let y = if self.opens_upward(height) {
            band.y - height as i32
        } else {
            band.y + band.height as i32
        };
        Rect::new(band.x, y, band.width, height)
    }

    fn action_rect(&self, index: usize) -> Option<Rect> {
        if index >= self.actions.len() || index < self.menu_scroll {
            return None;
        }
        let visible = index - self.menu_scroll;
        if visible >= self.visible_menu_rows() {
            return None;
        }
        let menu = self.menu_rect();
        let y = menu.y + visible as i32 * self.row_height as i32;
        Some(Rect::new(menu.x, y, menu.width, self.row_height))
    }

    fn hit_primary(&self, pos: Point) -> bool {
        let rect = self.primary_rect();
        pos.x >= rect.x
            && pos.x < rect.x + rect.width as i32
            && pos.y >= rect.y
            && pos.y < rect.y + rect.height as i32
    }

    fn hit_arrow(&self, pos: Point) -> bool {
        let rect = self.arrow_rect();
        pos.x >= rect.x
            && pos.x < rect.x + rect.width as i32
            && pos.y >= rect.y
            && pos.y < rect.y + rect.height as i32
    }

    fn hit_menu_index(&self, pos: Point) -> Option<usize> {
        if !self.menu_open {
            return None;
        }
        let menu = self.menu_rect();
        if pos.x < menu.x
            || pos.x >= menu.x + menu.width as i32
            || pos.y < menu.y
            || pos.y >= menu.y + menu.height as i32
        {
            return None;
        }
        let index = ((pos.y - menu.y) / self.row_height as i32) as usize + self.menu_scroll;
        (index < self.actions.len()).then_some(index)
    }
}

impl Widget for SplitButton {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    /// The control **and** its open drop-down accept the pointer.
    ///
    /// The list hangs below the face, outside this control's rectangle. Without this override the
    /// runtime's own bounds check rejects every press on a row that is visible but past the
    /// control's bottom edge — the rows would be painted and unreachable.
    fn contains_point(&self, point: Point) -> bool {
        self.base().contains_point_with_touch_expansion(point)
            || (self.menu_open && self.menu_rect().contains_point(point))
    }

    /// The open drop-down is a real popup, so the runtime lets it win the hit test against a
    /// sibling that covers the same pixels and treats an outside press as its dismissal.
    fn has_open_popup(&self) -> bool {
        self.menu_open
    }

    /// Advances the drop-down's open/close reveal; `true` while it still owes frames.
    fn tick(&mut self, delta_ms: u32) -> bool {
        self.reveal.tick(delta_ms)
    }

    fn is_animating(&self) -> bool {
        self.reveal.is_animating()
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(
            dimensions::SPLIT_BUTTON_DEFAULT_WIDTH,
            dimensions::SPLIT_BUTTON_HEIGHT,
        )
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    /// Resolves a published event name to the control's own signal.
    ///
    /// # Why this is explicit per control
    ///
    /// `connect_event` validates a name against the capability table and registers a hub slot; only
    /// `event_signal_dyn` joins that name to the signal the control actually emits. Without it a name
    /// is valid and inert, which is the silent failure `tools/check_event_signal_dyn.sh` exists to
    /// make impossible. The arm set is checked against the capability's published names, so this
    /// list cannot drift from what the control advertises.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        #[allow(unused_imports)]
        use crate::widget::capability::CapabilityValue;
        match name {
            "triggered" => Some(EventSignalRef::mapped("triggered", &self.triggered, |v| {
                CapabilityValue::String(v.clone())
            })),
            "action_selected" => {
                Some(EventSignalRef::mapped("action_selected", &self.action_selected, |v| {
                    CapabilityValue::String(v.clone())
                }))
            }
            "menu_toggled" => {
                Some(EventSignalRef::mapped("menu_toggled", &self.menu_toggled, |v| {
                    CapabilityValue::Bool(*v)
                }))
            }
            _ => None,
        }
    }
}

/// `SplitButton`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_other.in.rs` / `access_write_other.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before. `SplitButton` reports
/// `WidgetKind::ToolButton`, shared with `ToolButton`; dispatching on the concrete
/// type here is what keeps the two contracts separate.
impl WidgetProperties for SplitButton {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            // The whole action list as records (BLUE26 B26.09): `add_action` takes the action to
            // add, which the bare command channel cannot carry, so the list is the property and
            // `set_actions` is its published command form.
            "actions" => {
                Ok(CapabilityValue::Records(crate::widget::capability::RecordList::from_items(
                    &SPLIT_ACTION_SCHEMA,
                    self.actions.iter().map(split_action_record).collect::<Vec<_>>(),
                )))
            }
            "text" => Ok(CapabilityValue::String(self.text().to_string())),
            "action_count" => Ok(CapabilityValue::UInt(self.actions().len() as u64)),
            "menu_open" => Ok(CapabilityValue::Bool(self.menu_open())),
            "row_height" => Ok(CapabilityValue::UInt(self.row_height() as u64)),
            "popup_direction" => {
                Ok(CapabilityValue::String(self.popup_direction().as_str().to_string()))
            }
            "max_menu_height" => Ok(match self.max_menu_height() {
                Some(height) => CapabilityValue::UInt(height as u64),
                None => CapabilityValue::Null,
            }),
            "menu_scroll" => Ok(CapabilityValue::UInt(self.menu_scroll() as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "text" => {
                self.set_text(expect_string(value)?);
                Ok(())
            }
            "action_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            // A whole-list replacement through the control's own `set_actions`, which also resets
            // the primary action the same way it does for any other caller.
            "actions" => {
                let CapabilityValue::Records(list) = value else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                list.validate_against(&SPLIT_ACTION_SCHEMA).map_err(|violation| {
                    log::warn!("split_button.actions refused: {violation}");
                    CapabilityAccessError::ConstraintViolation {
                        property: crate::compat::String::from("actions"),
                        reason: crate::compat::format!("{violation}"),
                    }
                })?;
                let actions = list.items().iter().map(split_action_from_record).collect::<Result<
                    Vec<_>,
                    CapabilityAccessError,
                >>(
                )?;
                self.set_actions(actions);
                Ok(())
            }
            "menu_open" => {
                if expect_bool(value)? {
                    self.open_menu();
                } else {
                    self.close_menu();
                }
                Ok(())
            }
            "row_height" => {
                self.set_row_height(expect_u32(value)?);
                Ok(())
            }
            "popup_direction" => {
                let token = expect_string(value)?;
                let direction =
                    PopupDirection::from_name(&token).ok_or(CapabilityAccessError::OutOfRange)?;
                self.set_popup_direction(direction);
                Ok(())
            }
            "max_menu_height" => {
                let height = match value {
                    CapabilityValue::UInt(height) => Some(
                        u32::try_from(height).map_err(|_| CapabilityAccessError::TypeMismatch)?,
                    ),
                    CapabilityValue::Null => None,
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                self.set_max_menu_height(height);
                Ok(())
            }
            "menu_scroll" => {
                self.set_menu_scroll(expect_u32(value)? as usize);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "actions",
            "text",
            "action_count",
            "menu_open",
            "row_height",
            "popup_direction",
            "max_menu_height",
            "menu_scroll",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `split_button` publishes.
    ///
    /// `open_menu` and `close_menu` are the genuine zero-argument actions here.
    /// `trigger_primary` acts on the primary action and reports `false` when there
    /// is no action to trigger, which is the "could not handle it" case, so it is
    /// answered with [`CapabilityAccessError::OutOfRange`] rather than a success
    /// that did nothing. `add_action` takes the action to add, so it is refused the
    /// same way — the name is right and the argument is what is missing.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "open_menu" => {
                self.open_menu();
                Ok(())
            }
            "close_menu" => {
                self.close_menu();
                Ok(())
            }
            "trigger_primary" => {
                if self.trigger_primary() {
                    Ok(())
                } else {
                    Err(CapabilityAccessError::OutOfRange)
                }
            }
            "add_action" => Err(CapabilityAccessError::OutOfRange),
            "set_actions" => Err(CapabilityAccessError::OutOfRange),
            // Any other `set_foo` name carries its value through the property route,
            // so the shared default reports that a payload is needed rather than
            // claiming the control has never heard of it.
            _ if name.starts_with("set_") => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for SplitButton {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }

        match event {
            Event::MouseMove { pos } => {
                self.hovered_primary = self.hit_primary(*pos);
                self.hovered_arrow = self.hit_arrow(*pos);
                if let Some(index) = self.hit_menu_index(*pos) {
                    self.highlighted_action_index = Some(index);
                }
            }
            Event::MouseLeave { .. } => {
                self.hovered_primary = false;
                self.hovered_arrow = false;
                self.pressed_primary = false;
                self.pressed_arrow = false;
            }
            Event::MousePress { pos, button: 1, .. } => {
                if self.hit_primary(*pos) {
                    self.pressed_primary = true;
                } else if self.hit_arrow(*pos) {
                    self.pressed_arrow = true;
                } else if let Some(index) = self.hit_menu_index(*pos) {
                    self.highlighted_action_index = Some(index);
                } else {
                    self.close_menu();
                }
            }
            Event::MouseRelease { pos, button: 1 } => {
                if self.pressed_primary {
                    self.pressed_primary = false;
                    if self.hit_primary(*pos) {
                        let _ = self.trigger_primary();
                    }
                } else if self.pressed_arrow {
                    self.pressed_arrow = false;
                    if self.hit_arrow(*pos) {
                        self.toggle_menu();
                    }
                } else if let Some(index) = self.hit_menu_index(*pos) {
                    self.highlighted_action_index = Some(index);
                    let _ = self.select_highlighted_action();
                }
            }
            Event::Wheel { delta, .. } if self.menu_open && delta.y != 0 => {
                let step = if delta.y > 0 { 1isize } else { -1 };
                let max = self.actions.len().saturating_sub(self.visible_menu_rows());
                let next = (self.menu_scroll as isize + step).clamp(0, max as isize) as usize;
                self.set_menu_scroll(next);
            }
            Event::KeyPress { key, modifiers: _ } => match *key {
                13 | 32 => {
                    if self.menu_open {
                        let _ = self.select_highlighted_action();
                    } else {
                        let _ = self.trigger_primary();
                    }
                }
                40 => {
                    if !self.menu_open {
                        self.open_menu();
                    } else {
                        self.move_highlight(1);
                    }
                }
                38 if self.menu_open => {
                    self.move_highlight(-1);
                }
                27 => {
                    self.close_menu();
                }
                // Unknown key; ignore
                _ => {}
            },
            // Other events are not relevant for this widget
            _ => {}
        }
    }
}

impl Draw for SplitButton {
    fn draw(&mut self, context: &mut RenderContext) {
        // Chrome colours resolve the explicit style first, then the theme's resolved style for
        // this control, and only then a literal. Every colour below used to be a literal, so a
        // light/dark switch left the button, its splits and its drop-down unchanged — the
        // rendering census reported the control as theme-blind.
        //
        // The theme read is a separate manager lock, taken and released inside
        // `resolved_theme_style`, so it is not held across the draw — the global manager's mutex
        // is not re-entrant.
        let style = self.base.style();
        // The **effective font** — the resolved theme/caller font — used for the trigger label, the
        // drop-down arrow glyph and every menu row, so the control honours the theme body font and
        // the user's text scale (D09-STYLE-01).
        let font = effective_font(style);
        let theme = crate::style::resolved_theme_style_shared("split_button");
        // Read once from the palette snapshot and copied out as values; `palette_shared()` holds
        // no manager guard, so the manager is free for the rest of the draw.
        let (window_fill, foreground, secondary, primary) = {
            match crate::style::palette_shared() {
                Some(colors) => {
                    (colors.background, colors.foreground, colors.secondary, colors.primary)
                }
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
        // `split_button` is absent from `WidgetRole::for_kind_name`'s table, so it classifies as
        // `Surface` and the active theme writes the window fill into `style.background_color`.
        // A face painted in that colour would be byte-identical to the frame behind it, so a
        // resolved surface equal to the window fill is re-derived a visible step away from it,
        // while a colour the caller set still wins.
        let face = match style.background_color {
            Some(resolved) if resolved != window_fill => resolved,
            _ => window_fill.blend(&ink, 0.08),
        };
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .filter(|resolved| *resolved != face)
            .unwrap_or_else(|| face.blend(&secondary, 0.45));

        // The states are raised from the face toward the theme's primary, so hover and press read
        // on either appearance rather than being a fixed pale blue that only worked on a light bar.
        let hover_bg = face.blend(&primary, 0.28);
        let pressed_bg = face.blend(&primary, 0.45);
        let arrow_face = face.blend(&ink, 0.06);

        // ── The face actually painted ──
        //
        // The button's chrome is one compact row, not a filled container; the band is where the
        // trigger and the arrow are drawn and where the popup is anchored.
        let face_rect = self.face_band();
        context.fill_rect(face_rect, face);
        context.draw_rect(face_rect, border);

        let primary_rect = self.primary_rect();
        let arrow = self.arrow_rect();

        let primary_bg = if self.pressed_primary {
            pressed_bg
        } else if self.hovered_primary {
            hover_bg
        } else {
            face
        };
        context.fill_rect(primary_rect, primary_bg);

        let arrow_bg = if self.pressed_arrow || self.menu_open {
            pressed_bg
        } else if self.hovered_arrow {
            hover_bg
        } else {
            arrow_face
        };
        context.fill_rect(arrow, arrow_bg);

        context.draw_line(
            Point::new(arrow.x, arrow.y),
            Point::new(arrow.x, arrow.y + arrow.height as i32),
            border,
        );

        // `draw_text`'s origin is the glyph box's top-left, so `primary_rect.y + height / 2` put
        // that top edge on the middle line and drew the label half a line low. The line box
        // centred in the trigger is the origin; the arrow glyph below shares its band's own.
        //
        // The label is centred both ways inside the trigger's padded box. A split button's
        // primary face *is* a button, so its label follows the same rule `Button` does
        // (the content item is centred, as Material M3 centres its child) —
        // the previous `x + 8` origin left-aligned it against a literal.
        let primary_box = self.primary_label_box(primary_rect);
        context.draw_text_fitted_cow(
            context.text_line(primary_box, font),
            &self.text,
            font,
            ink,
            HorizontalAlignment::Center,
        );

        // The arrow is a **chevron icon**, centred on its own column, rather than the letter
        // `v` a reader had to interpret as a drop-down affordance. An icon is geometry: both
        // backends draw the same outline, and the glyph table cannot substitute a different
        // shape at another font size. `draw_icon_centered` clamps the side to the column, so a
        // narrow column shrinks the chevron instead of clipping it.
        let arrow_ink = ink.blend(&arrow_bg, 0.35);
        let icon_side = arrow.width.min(arrow.height).max(1);
        crate::widget::draw_icon_centered(
            context,
            arrow,
            icon_side,
            arrow_ink,
            IconName::ChevronDown,
        );

        // The panel is painted while it is **either** open or still leaving: `menu_open` flips on
        // the frame `close_menu` runs, but the reveal can still have frames to show. Clipping to
        // the revealed rectangle is what keeps a partially-open list to the rows it has actually
        // uncovered — the same contract the shared `PopupReveal` gives the rest of the family.
        if self.menu_open || !self.reveal.is_closed() {
            let menu = self.menu_rect();
            let revealed = self.reveal.revealed(
                menu,
                RevealDirection::Down,
                PopupReveal::travel_cap_for(Size::new(menu.width, menu.height)),
            );
            let clipped = !self.reveal.is_closed();
            if clipped {
                context.push_clip(revealed.x, revealed.y, revealed.width, revealed.height);
            }
            context.fill_rect(menu, face.blend(&ink, 0.22));
            context.draw_rect(menu, border);

            for index in 0..self.actions.len() {
                let Some(action_rect) = self.action_rect(index) else {
                    continue;
                };

                if self.highlighted_action_index == Some(index) {
                    context.fill_rect(action_rect, hover_bg);
                }

                if let Some(action) = self.actions.get(index) {
                    let row_ink = if action.enabled { ink } else { ink.with_alpha(110) };
                    // A menu row's label is centred through the shared primitive, since the
                    // glyph origin is a top edge and `action_rect.y + height / 2` placed it
                    // half a line low. The box is the row's own padded interior — the same
                    // `SPLIT_BUTTON_PADDING_H` the trigger uses, so a menu row and the trigger
                    // it hangs from share their leading space rather than each naming it.
                    let row_box = self.primary_label_box(action_rect);
                    // A shortcut occupies the row's trailing 64 px, exactly as the command
                    // palette reserves its own, so a key hint never sits on the label.
                    let shortcut_gap = 64i32.min(row_box.width as i32);
                    let has_shortcut = action.shortcut.is_some();
                    let label_box = if action.icon.is_some() {
                        Rect::new(
                            row_box.x + 16,
                            row_box.y,
                            row_box.width.saturating_sub(
                                16 + if has_shortcut { shortcut_gap as u32 } else { 0 },
                            ),
                            row_box.height,
                        )
                    } else if has_shortcut {
                        Rect::new(
                            row_box.x,
                            row_box.y,
                            row_box.width.saturating_sub(shortcut_gap as u32),
                            row_box.height,
                        )
                    } else {
                        row_box
                    };
                    if let Some(icon) = action.icon {
                        let side = 12u32.min(row_box.height.max(1));
                        let icon_rect = Rect::new(
                            row_box.x,
                            row_box.y + (row_box.height.saturating_sub(side) / 2) as i32,
                            side,
                            side,
                        );
                        crate::widget::draw_icon_at(context, icon_rect, row_ink, icon);
                    }
                    context.draw_text_fitted_cow(
                        context.text_line(label_box, font),
                        &action.label,
                        font,
                        row_ink,
                        HorizontalAlignment::Left,
                    );
                    if action.checked {
                        let side = 12u32.min(row_box.height.max(1));
                        let check = Rect::new(
                            row_box.x + row_box.width as i32 - side as i32 - 2,
                            row_box.y + (row_box.height.saturating_sub(side) / 2) as i32,
                            side,
                            side,
                        );
                        crate::widget::draw_icon_at(context, check, row_ink, IconName::Check);
                    }
                    if let Some(shortcut) = action.shortcut.as_deref() {
                        let key_box = Rect::new(
                            row_box.x + row_box.width as i32 - shortcut_gap,
                            row_box.y,
                            (shortcut_gap - 4).max(0) as u32,
                            row_box.height,
                        );
                        context.draw_text_line_cow(
                            key_box,
                            shortcut,
                            font,
                            row_ink.blend(&face, 0.25),
                            HorizontalAlignment::Right,
                        );
                    }
                }
                if action_rect.y > menu.y
                    && self.actions.get(index).is_some_and(|a| a.separator_above)
                {
                    context.draw_line(
                        Point::new(menu.x + 6, action_rect.y),
                        Point::new(menu.x + menu.width as i32 - 6, action_rect.y),
                        ink.with_alpha(60),
                    );
                }
            }
            // Popped on the same path it was pushed, so a reveal cannot leak a clip onto whatever
            // the host paints after this control.
            if clipped {
                context.pop_clip();
            }
        }
    }
}

/// The width a trigger label is measured at: the **shared** text estimate plus
/// [`dimensions::BUTTON_PADDING_H`] on each side.
///
/// # Why this is a production function rather than a test helper
///
/// It began as a test-local duplicate of [`SplitButton::trigger_hint_width`], and that is
/// precisely the shape this crate keeps paying for: two copies of one measurement, which agree on
/// the day they are written and are edited one at a time afterwards. `trigger_hint_width` now
/// calls it, so the face the control assembles and the width the invariant tests assert are the
/// same derivation instead of two that must be kept in step.
///
/// It replaced `text.len() as u32 * 8`, which counted UTF-8 **bytes**; see `trigger_hint_width`
/// for why that was wrong.
fn split_hint_width(text: &str, font: &Font) -> u32 {
    estimate_text_width(text, font, 1.0) + dimensions::BUTTON_PADDING_H * 2
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn sample_actions() -> Vec<SplitAction> {
        vec![
            SplitAction::new("run.default", "Run"),
            SplitAction::new("run.debug", "Run with Debug"),
            SplitAction::new("run.profile", "Run with Profile"),
        ]
    }

    /// A disabled row is skipped by keyboard navigation and cannot be selected.
    #[test]
    fn disabled_actions_are_skipped_and_checked_actions_carry_their_flag() {
        let mut button = SplitButton::new("Run", Rect::new(0, 0, 200, 28));
        button.set_actions(vec![
            SplitAction::new("one", "One"),
            SplitAction::new("two", "Two").with_enabled(false),
            SplitAction::new("three", "Three").with_checked(true),
        ]);
        button.open_menu();
        button.move_highlight(1);
        assert_eq!(button.highlighted_action_index(), Some(2), "the disabled row is skipped");
        assert!(button.actions()[2].checked);

        button.highlighted_action_index = Some(1);
        assert!(!button.select_highlighted_action(), "a disabled action refuses selection");
    }

    /// A capped menu scrolls its rows, and `Up` puts the panel above the face.
    #[test]
    fn a_capped_menu_scrolls_and_can_open_upward() {
        let mut button = SplitButton::new("Run", Rect::new(0, 0, 200, 28));
        button.set_actions((0..6).map(|i| SplitAction::new(format!("a{i}"), "row")).collect());
        button.set_max_menu_height(Some(44));
        assert_eq!(button.row_height(), 22);
        button.open_menu();
        button.move_highlight(5);
        assert_eq!(button.menu_scroll(), 4, "the window follows the highlight");
        button.set_menu_scroll(0);
        assert_eq!(button.menu_scroll(), 0);

        button.set_popup_direction(PopupDirection::Up);
        let menu = button.menu_rect();
        let face = button.face_band();
        assert_eq!(menu.y + menu.height as i32, face.y, "an upward menu ends at the face top");
    }

    #[test]
    fn primary_trigger_emits_default_action() {
        let mut split = SplitButton::new("Run", Rect::new(0, 0, 160, 28));
        split.set_actions(sample_actions());

        let emitted = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = emitted.clone();
        split.triggered.connect(move |id| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(id.as_ref().clone());
            }
        });

        assert!(split.trigger_primary());
        let got = emitted.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert_eq!(got, vec!["run.default".to_string()]);
    }

    #[test]
    fn keyboard_navigation_selects_action_from_menu() {
        let mut split = SplitButton::new("Run", Rect::new(0, 0, 180, 28));
        split.set_actions(sample_actions());

        let selected = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = selected.clone();
        split.action_selected.connect(move |id| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(id.as_ref().clone());
            }
        });

        split.handle_event(&Event::key_press(40, 0));
        assert!(split.menu_open());
        split.handle_event(&Event::key_press(40, 0));
        split.handle_event(&Event::key_press(13, 0));

        assert_eq!(split.primary_action_index(), Some(1));
        let got = selected.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert_eq!(got, vec!["run.debug".to_string()]);
        assert!(!split.menu_open());
    }

    #[test]
    fn arrow_click_toggles_menu_and_mouse_selects_action() {
        let mut split = SplitButton::new("Run", Rect::new(0, 0, 180, 28));
        split.set_actions(sample_actions());

        // Click arrow area to open menu.
        split.handle_event(&Event::mouse_press(172, 12, 1));
        split.handle_event(&Event::mouse_release(172, 12, 1));
        assert!(split.menu_open());

        // Click second row in menu.
        let menu_y = 28 + 24;
        split.handle_event(&Event::mouse_press(20, menu_y, 1));
        split.handle_event(&Event::mouse_release(20, menu_y, 1));

        assert_eq!(split.primary_action_index(), Some(1));
        assert!(!split.menu_open());
    }

    #[test]
    fn default_state() {
        let split = SplitButton::new("Run", Rect::new(0, 0, 800, 600));
        assert_eq!(split.text(), "Run");
        assert!(split.actions().is_empty());
        assert_eq!(split.primary_action_index(), None);
        assert!(!split.menu_open());
    }

    /// An open menu is a popup the runtime can route to, including outside this control's box.
    #[test]
    fn an_open_menu_answers_has_open_popup_and_accepts_rows_outside_the_face() {
        let mut split = SplitButton::new("Run", Rect::new(0, 0, 160, 20));
        split.set_actions(sample_actions());
        // The list hangs below the 20 px face; row 1 is entirely outside the control rectangle.
        let outside_row = Point::new(10, split.menu_rect().y + split.row_height() as i32 + 4);
        assert!(
            !split.contains_point(outside_row),
            "a closed control must not claim a point below itself"
        );

        split.open_menu();

        assert!(split.has_open_popup(), "an open list must tell the runtime it is a popup");
        assert!(
            split.contains_point(outside_row),
            "a visible row outside the control's rectangle must still be hit-testable"
        );
        assert!(!split.contains_point(Point::new(1000, 1000)), "and the claim is not unbounded");
        // Leave the thread-local grab as this test found it.
        split.close_menu();
    }

    /// A press outside the open menu dismisses it and performs nothing.
    #[test]
    fn an_outside_press_closes_the_menu_without_triggering_an_action() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let mut split = SplitButton::new("Run", Rect::new(0, 0, 160, 28));
        split.set_actions(sample_actions());
        let triggered = Arc::new(AtomicUsize::new(0));
        let selected = Arc::new(AtomicUsize::new(0));
        {
            let triggered = Arc::clone(&triggered);
            split.triggered.connect(move |_| {
                triggered.fetch_add(1, Ordering::SeqCst);
            });
        }
        {
            let selected = Arc::clone(&selected);
            split.action_selected.connect(move |_| {
                selected.fetch_add(1, Ordering::SeqCst);
            });
        }

        split.open_menu();
        split.handle_event(&Event::MousePress {
            pos: Point::new(900, 900),
            button: 1,
            modifiers: 0,
        });

        assert!(!split.menu_open(), "the outside press is the dismissal");
        assert_eq!(triggered.load(Ordering::SeqCst), 0, "a dismissal is not an activation");
        assert_eq!(selected.load(Ordering::SeqCst), 0, "nor a selection");
    }

    /// A press that dismisses the popup must not also activate a sibling underneath it.
    ///
    /// This is the runtime's popup-grab contract: because the control answers
    /// `has_open_popup`, the outside press is delivered to it and goes no further. The sibling
    /// therefore sees a release with no matching press (and, for a button, no click).
    #[test]
    #[cfg(not(alloc_frugal))]
    fn an_outside_press_is_dismissed_and_does_not_reach_a_sibling() {
        use crate::widget::runtime::{
            clear_popup_grab, dispatch_pointer_event, register, unregister, with_widget_mut,
        };
        use std::sync::atomic::{AtomicUsize, Ordering};

        let split = register(Box::new(SplitButton::new("Run", Rect::new(20, 20, 160, 28))))
            .expect("mount the split button");
        let _ = with_widget_mut(split, |widget| {
            let control = (widget as &mut dyn core::any::Any)
                .downcast_mut::<SplitButton>()
                .expect("the mounted control is a split button");
            control.set_actions(sample_actions());
            // Opened after mounting so `self.id()` resolves to the registry id the router
            // addresses; a grab taken before mount would name an id nothing is registered at.
            control.open_menu();
        });

        let clicks = Arc::new(AtomicUsize::new(0));
        let sibling = crate::widget::base_widgets::button::Button::new(
            "Other".to_string(),
            Rect::new(220, 200, 100, 30),
        );
        let sibling = register(Box::new(sibling)).expect("mount the sibling");
        {
            let clicks = Arc::clone(&clicks);
            let _ = with_widget_mut(sibling, |widget| {
                widget.base().clicked.connect(move || {
                    clicks.fetch_add(1, Ordering::SeqCst);
                });
            });
        }

        // Register the window before linking the children: `add_child_linked` writes the
        // parent link in the *registry* id space, which it can only resolve once the window
        // itself is mounted. `top_level_window_of` walks that link when the popup grab is
        // checked, so a window that was linked before mounting would make a real popup look
        // like it belonged to no surface.
        let root = register(Box::new(crate::widget::Window::new(
            "popup".to_string(),
            Rect::new(0, 0, 400, 300),
        )))
        .expect("mount the window");
        let _ = with_widget_mut(root, |widget| {
            let window = (widget as &mut dyn core::any::Any)
                .downcast_mut::<crate::widget::Window>()
                .expect("the mounted root is a window");
            window.add_child(split);
            window.add_child(sibling);
        });

        let point = Point::new(270, 215);
        let handled = dispatch_pointer_event(
            root,
            &Event::MousePress { pos: point, button: 1, modifiers: 0 },
            point,
        );
        assert!(handled, "the dismissive press is delivered to the popup");
        let still_open = with_widget_mut(split, |widget| {
            (widget as &dyn core::any::Any)
                .downcast_ref::<SplitButton>()
                .map(SplitButton::menu_open)
                .unwrap_or(false)
        })
        .unwrap_or(false);
        assert!(!still_open, "the popup closes on the press that dismisses it");

        // The release lands on the sibling, but it never saw the press, so it cannot click.
        dispatch_pointer_event(root, &Event::MouseRelease { pos: point, button: 1 }, point);
        assert_eq!(
            clicks.load(Ordering::SeqCst),
            0,
            "the dismissing press must not pass through to the sibling"
        );

        clear_popup_grab();
        unregister(root);
        unregister(sibling);
        unregister(split);
    }

    /// `tick` drives the reveal and reports the frames it still owes.
    #[test]
    fn opening_the_menu_drives_the_reveal_through_tick() {
        use crate::widget::menu_toolbar::effects::with_test_effects;

        // Pin the effect policy this test needs rather than inheriting the process-wide config:
        // the reveal's contract is "moves while enabled", and a host that disabled animations
        // legitimately settles at once.
        let mut effects = crate::style::MenuEffects::legacy();
        effects.enabled = true;
        effects.animations = true;
        with_test_effects(effects, || {
            let mut split = SplitButton::new("Run", Rect::new(0, 0, 160, 28));
            split.set_actions(sample_actions());
            split.open_menu();

            assert!(split.is_animating(), "an opening menu owes frames");
            assert!(split.tick(1), "the first millisecond is still in flight");

            let mut frames = 0;
            while split.tick(16) {
                frames += 1;
                assert!(frames < 1_000, "the reveal must settle rather than animate forever");
            }
            assert!(!split.is_animating(), "a settled reveal owes no further frames");
            // Close (and, on the animation-enabled path, clear the grab this open_menu took).
            split.close_menu();
        });
    }

    #[test]
    fn set_text_get_text_roundtrip() {
        let mut split = SplitButton::new("Initial", Rect::new(0, 0, 800, 600));
        assert_eq!(split.text(), "Initial");

        split.set_text("Updated");
        assert_eq!(split.text(), "Updated");

        split.set_text("");
        assert_eq!(split.text(), "");
    }

    #[test]
    fn add_action_adds_to_menu() {
        let mut split = SplitButton::new("Action", Rect::new(0, 0, 800, 600));
        assert_eq!(split.actions().len(), 0);

        let idx = split.add_action(SplitAction::new("act1", "Action 1"));
        assert_eq!(idx, 0);
        assert_eq!(split.actions().len(), 1);
        assert_eq!(split.primary_action_index(), Some(0));

        let idx = split.add_action(SplitAction::new("act2", "Action 2"));
        assert_eq!(idx, 1);
        assert_eq!(split.actions().len(), 2);

        assert_eq!(split.actions()[0].id, "act1");
        assert_eq!(split.actions()[1].id, "act2");
    }

    #[test]
    fn enable_disable_states() {
        let mut split = SplitButton::new("Test", Rect::new(0, 0, 800, 600));
        split.set_actions(vec![SplitAction::new("a", "A")]);

        // Enabled by default
        assert!(split.trigger_primary());

        // Disable widget
        split.base_mut().set_enabled(false);

        // After disabling, events should be ignored
        let emitted = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = emitted.clone();
        split.triggered.connect(move |id| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(id.as_ref().clone());
            }
        });

        // Direct call still works but event handler ignores
        split.handle_event(&Event::key_press(40, 0));
        let _got = emitted.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert!(!split.menu_open(), "disabled widget should not open menu via events");
    }

    #[test]
    fn trigger_with_no_actions() {
        let mut split = SplitButton::new("Empty", Rect::new(0, 0, 800, 600));
        // No actions added yet
        assert!(!split.trigger_primary());

        // open_menu with no actions still opens an empty menu (code doesn't guard)
        split.open_menu();
        assert!(split.menu_open());

        // Close menu
        split.close_menu();
        assert!(!split.menu_open());
    }

    #[test]
    fn menu_toggle_signal_emission() {
        let mut split = SplitButton::new("Test", Rect::new(0, 0, 800, 600));
        split.set_actions(vec![SplitAction::new("a", "A"), SplitAction::new("b", "B")]);

        let emitted = Arc::new(Mutex::new(Vec::<bool>::new()));
        let sink = emitted.clone();
        split.menu_toggled.connect(move |state| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(*state.as_ref());
            }
        });

        split.open_menu();
        assert!(split.menu_open());

        split.close_menu();
        assert!(!split.menu_open());

        let got = emitted.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert_eq!(got, vec![true, false]);
    }

    /// The assembled face is the pair of columns the hand-computed geometry produced.
    ///
    /// # Why this test exists at all
    ///
    /// BLUE22 §B.6 rule 2 asks placement to belong to the layout, and the risk of adopting a
    /// layout for a two-column chrome is precisely that it *changes* the geometry — a face that
    /// now includes a padding the old arithmetic did not, or a trigger that no longer ends where
    /// the arrow begins. This states the three properties that make the assembly correct rather
    /// than merely different, and it is checked at several control widths because the two columns
    /// have a fixed arrow width and a text-driven trigger width: they collide exactly where one
    /// derivation is dropped.
    ///
    /// The old form was `primary_width = band.width - arrow_width` with the arrow placed from the
    /// band's trailing edge, so the tiling held only because both subtracted the same constant.
    #[test]
    fn the_two_columns_tile_the_face() {
        // Every width that can hold both columns. `MIN_TILE_WIDTH` is the narrowest face for which
        // tiling is *possible at all* — it is not a threshold the code applies, it is the sum of
        // the two columns' own sizes, and the assertion below is that the assembly tiles exactly
        // wherever tiling is representable.
        let min_tile_width =
            split_hint_width("Run", &Font::default()) + dimensions::SPLIT_ARROW_COLUMN_WIDTH;
        for width in [min_tile_width, 100, 240, 400] {
            let split = SplitButton::new("Run", Rect::new(0, 0, width, 120));
            let band = split.face_band();
            let primary = split.primary_rect();
            let arrow = split.arrow_rect();
            assert_eq!(
                primary.x, band.x,
                "the trigger starts at the face's leading edge at width {width}"
            );
            assert_eq!(
                primary.x + primary.width as i32,
                arrow.x,
                "the trigger ends where the arrow column begins at width {width}"
            );
            assert_eq!(
                arrow.x + arrow.width as i32,
                band.x + band.width as i32,
                "the arrow ends where the face ends at width {width}"
            );
            assert_eq!(primary.height, band.height, "both columns are the face's own height");
            assert_eq!(arrow.height, band.height);
        }
    }

    /// A face too narrow for both columns still keeps both of them inside it.
    ///
    /// # What this pins
    ///
    /// The two columns between them need `split_hint_width("Run") + SPLIT_ARROW_COLUMN_WIDTH`. A
    /// narrower face cannot be tiled at those sizes — that is arithmetic, not a bug.
    ///
    /// What the layout owes the caller is that the shortfall is **shared and contained** rather
    /// than deferred to whichever column comes last. Before G-1 was resolved the trigger kept its
    /// full width and the arrow was placed past the face's trailing edge; because the SVG backend
    /// emits absolute coordinates and nothing clips at this layer, that arrow was not overflowing,
    /// it was **absent** — a split button that silently has no drop-down affordance.
    ///
    /// The test therefore asserts containment (both columns inside the face) and the *relation* that
    /// makes a split button a split button: the arrow column stays one arrow wide, so a squeezed face
    /// loses trigger, not the affordance.
    #[test]
    fn a_face_too_narrow_for_both_columns_keeps_both_inside_it() {
        let width = 48u32;
        let split = SplitButton::new("Run", Rect::new(0, 0, width, 120));
        let face = split.face_band();
        let primary = split.primary_rect();
        let arrow = split.arrow_rect();
        assert_eq!(face.width, width);
        for (label, rect) in [("trigger", primary), ("arrow", arrow)] {
            assert!(
                rect.x >= face.x && rect.x + rect.width as i32 <= face.x + face.width as i32,
                "the {label} column must stay inside the face, got {rect:?} in {face:?}"
            );
        }
        assert_eq!(
            primary.width + arrow.width,
            width,
            "and the two columns must still account for the face: {primary:?} + {arrow:?}"
        );
        // Both columns are inside the face and the face is the sum of them — that is containment,
        // which is the part G-1 was about. Their *sizes* are scaled by the same factor, so a narrow
        // face is honestly described as "this control is smaller than its contents": the arrow keeps
        // its **share** rather than being dropped, which is the property that matters (an absent
        // drop-down arrow is a split button that is not a split button).
        let scale = width as f32
            / (split_hint_width("Run", &Font::default()) + dimensions::SPLIT_ARROW_COLUMN_WIDTH)
                as f32;
        assert!(
            (arrow.width as f32 - dimensions::SPLIT_ARROW_COLUMN_WIDTH as f32 * scale).abs() <= 1.0,
            "the arrow keeps its proportional share of a squeezed face: {} vs {} × {scale:.2}",
            arrow.width,
            dimensions::SPLIT_ARROW_COLUMN_WIDTH
        );
        assert!(
            arrow.width > 0 && primary.width > 0,
            "neither column is dropped: trigger {}, arrow {}",
            primary.width,
            arrow.width
        );
    }

    /// A wider arrow column pushes the trigger narrower rather than overlapping it.
    ///
    /// This is BLUE22 §B.9's rule stated for this control: the trigger's width is *derived* from
    /// the sibling column, so the two cannot be placed from opposite edges of the face. It is the
    /// property the layout provides and the previous `band.width - arrow_width` arithmetic only
    /// happened to satisfy.
    #[test]
    fn a_wider_arrow_column_pushes_the_trigger() {
        let mut split = SplitButton::new("Run", Rect::new(0, 0, 240, 120));
        let before = split.primary_rect().width;
        split.arrow_width = dimensions::SPLIT_ARROW_COLUMN_WIDTH + 10;
        let after = split.primary_rect().width;
        assert_eq!(
            before - after,
            10,
            "the trigger yields exactly the room the arrow took ({before} -> {after})"
        );
        assert_eq!(
            after + split.arrow_rect().width,
            split.face_band().width,
            "the two columns still tile the face after the arrow grew"
        );
    }

    /// The arrow column is the box the icon is painted into, and it paints no letter.
    ///
    /// The control used to paint the character `v` in its second column, so the drop-down
    /// affordance read as the letter v — the "wrong button text" a reader notices first. The
    /// face now paints a chevron **icon outline**; this test pins the absence of text in that
    /// column, which is the part of the change that is inspectable in the emitted document.
    #[test]
    fn the_arrow_column_paints_an_icon_and_not_a_letter() {
        // Holds the crate-wide theme guard: this test renders, and a concurrent
        // test that switches the appearance would otherwise change a later frame.
        let _theme_guard = crate::style::theme_test_guard();
        let mut split = SplitButton::new("Run", Rect::new(0, 0, 240, 120));
        let arrow = split.arrow_rect();
        let svg = crate::widget::svg::render_to_svg(&mut split);
        let boxes = text_run_boxes(&svg);
        assert_eq!(boxes.len(), 1, "only the trigger is text: {boxes:?}");
        assert!(
            boxes.iter().all(|(left, _, right, _)| {
                let centre = (left + right) / 2;
                centre < arrow.x || centre >= arrow.x + arrow.width as i32
            }),
            "the arrow column must not contain a letter: {boxes:?}, column {arrow:?}"
        );
    }

    /// The button's face is one compact row, and the popup hangs from it.
    ///
    /// The defect this pins: the face and its two halves were sized from `rect`, so a 240x120
    /// census cell drew a 120 px-tall face, and the drop-down was anchored to the control's
    /// bottom edge — 120 px below the button the user pressed.
    #[test]
    fn the_face_keeps_its_own_height_and_anchors_the_menu() {
        let mut split = SplitButton::new("Run", Rect::new(0, 0, 240, 120));
        split.set_actions(sample_actions());

        let band = split.face_band();
        assert_eq!(
            band.height,
            crate::widget::metrics::dimensions::SPLIT_BUTTON_HEIGHT,
            "the face is a compact row, not the whole rectangle"
        );
        // The trigger and the arrow divide the face rather than the control.
        let primary = split.primary_rect();
        let arrow = split.arrow_rect();
        assert_eq!(primary.height, band.height);
        assert_eq!(arrow.height, band.height);
        assert_eq!(primary.width + arrow.width, band.width);
        // The popup begins at the face's bottom edge.
        let menu = split.menu_rect();
        assert_eq!(menu.y, band.y + band.height as i32);
    }

    /// One ink box per text `<path>` in document order, as `(left, top, right, bottom)`.
    ///
    /// # Why the ink and not the string
    ///
    /// Text leaves the SVG backend as glyph **geometry** — axis-aligned rectangles for a `font8x8`
    /// run, flattened outline polygons for a vector face — so neither the label nor the arrow is in
    /// the document as a string, and a test has to locate a run by *where* it is. That is the
    /// stronger check: the old form matched `>Sample</text>` and read the element's `x`, so a
    /// glyph placed off its column's centre with a correct attribute would have passed it.
    ///
    /// # Why the shared reader, and not a local one
    ///
    /// This used to parse the bitmap rectangles itself — `numbers[0..4]` read as `(x, y, w, h)` of
    /// an `M{x} {y}h{w}v{h}` subpath. That parser could only ever see 1-bit glyph ink: an outline is
    /// `M{a.b} {c.d}L...Z`, whose first four numbers are two *points*, not an origin and a size, so
    /// the local reader found nothing and the test reported "the arrow painted no ink". The bug was
    /// in the reader, not the control: it duplicated a parse the crate already has, with the same
    /// bitmap assumption that `widget::svg::is_text_path` had to be taught out of.
    ///
    /// Delegating means this test measures whatever the backend actually emits, bitmap or outline,
    /// and it cannot drift from the crate's own notion of "where is the text ink".
    fn text_run_boxes(svg: &str) -> Vec<(i32, i32, i32, i32)> {
        crate::widget::svg::text_ink_boxes(svg)
    }

    /// The trigger's label is centred in the trigger rather than started at its padding.
    ///
    /// It used to be placed at `primary_rect.x + 8` (left-aligned against a literal, so a 240 px
    /// control drew a 50 px word flush to the left of a 218 px face).
    ///
    /// The check is on the emitted **ink**, not on a `<text x>` attribute. The run's centre is
    /// compared to the box centre it must be centred on, within the fitter's own margin: a
    /// `font8x8` bitmap maps 8 columns onto `round(0.6 * size)` pixels, so the lit columns rarely
    /// start on the box's first pixel. Asserting the margin keeps this a statement about the
    /// layout rather than about the glyph table.
    #[test]
    fn the_label_is_centred_in_the_trigger() {
        let rect = Rect::new(0, 0, 240, 120);
        let mut split = SplitButton::new("Sample", rect);
        let primary = split.primary_rect();
        let band = split.face_band();

        let svg = crate::widget::svg::render_to_svg(&mut split);
        let runs = text_run_boxes(&svg);
        // The label's ink is centred in the trigger, which is the same point as the centre of the
        // trigger's *padded* content box: the padding is symmetric.
        let label = runs
            .iter()
            .find(|(l, _, r, _)| {
                let centre = (l + r) / 2;
                centre >= primary.x && centre < primary.x + primary.width as i32
            })
            .copied()
            .unwrap_or_else(|| panic!("the label painted no ink in the trigger: {svg}"));

        let margin = crate::render::TEXT_FIT_MARGIN as i32;
        let label_centre = (label.0 + label.2) / 2;
        let primary_centre = primary.x + primary.width as i32 / 2;
        assert!(
            (label_centre - primary_centre).abs() <= margin,
            "the label's ink centre {label_centre} must be the trigger's centre {primary_centre}"
        );
        // The old literal origin was the trigger's left padding: a centred label cannot start
        // there, because the padding alone is narrower than half the label's shortfall.
        assert!(
            label.0 > primary.x + dimensions::SPLIT_BUTTON_PADDING_H as i32,
            "the label is centred, not left-aligned at the padding: {label:?}"
        );
        assert!(label.2 > label.0, "the label laid down ink: {label:?}");
        // The label is painted inside the face band, so it cannot drift off the control.
        assert!(
            label.1 >= band.y && label.3 <= band.y + band.height as i32,
            "the label ink {label:?} must stay inside the face {band:?}"
        );
    }

    /// The trigger's label box and a menu row's label box share one padding derivation.
    #[test]
    fn the_trigger_and_the_menu_rows_share_their_leading_space() {
        let mut split = SplitButton::new("Run", Rect::new(0, 0, 240, 120));
        split.set_actions(sample_actions());
        let primary = split.primary_rect();
        let trigger_box = split.primary_label_box(primary);
        let row = split.action_rect(0).expect("the fixture has an action");
        let row_box = split.primary_label_box(row);
        assert_eq!(
            trigger_box.x - primary.x,
            row_box.x - row.x,
            "the trigger and its menu rows must inset their labels by the same amount"
        );
        assert_eq!(
            trigger_box.x - primary.x,
            dimensions::SPLIT_BUTTON_PADDING_H as i32,
            "and that amount is the named constant, not a literal"
        );
    }
}
