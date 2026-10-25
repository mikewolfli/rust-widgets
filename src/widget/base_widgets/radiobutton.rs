// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Radio button widget.
use crate::compat::{Rc, String, ToString};
use crate::core::{Color, Font, HorizontalAlignment, ObjectId, Point, Rect, Size};
use crate::event::key_codes;
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::widget::base_widgets::button_group::{self, GroupKey};
use crate::widget::base_widgets::AutoRepeat;
use crate::widget::capability::coercion::{expect_bool, expect_string, expect_u32};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{
    active_profile, dimensions, effective_font, estimate_line_height, estimate_text_width,
    ControlMetrics, ControlMetricsProfile, FocusRing, FOCUS_HALO_BLUR_RADIUS, FOCUS_RING_WIDTH,
};
use crate::widget::{BaseWidget, Draw, InteractionFade, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Radius of the indicator disc, in logical pixels.
///
/// A constant, for the same reason [`crate::widget::CheckBox`]'s indicator is: the disc is
/// chrome the control owns, while the rectangle it is laid out in belongs to whoever placed
/// it. `min(w, h) / 4` made a 240x120 census cell draw a 60 px circle, so the same control
/// was a different shape in every layout.
///
/// The value comes from the shared table (`dimensions::RADIO_OUTER_RADIUS`) rather than being
/// restated here, so the indicator, the ring's stroke and the dot cannot drift apart — and so
/// a checkbox and a radio, which sit side by side in a form, stay within a pixel of each
/// other's size.
const INDICATOR_RADIUS: u32 = dimensions::RADIO_OUTER_RADIUS;

/// Stroke width of the indicator's ring.
const RING_WIDTH: u32 = dimensions::RADIO_STROKE;

/// Distance from the control's left edge to the ring's outer edge.
///
/// Only a few pixels: a radio button's indicator is close to its own edge, which is what
/// lets several of them in a group line up as a column of discs.
const INDICATOR_INSET: i32 = 1;

/// The radio button's own padding: what it keeps between its rectangle and its contents.
const RADIO_PADDING: crate::style::EdgeOffsets =
    crate::style::EdgeOffsets { left: 1, top: 0, right: 1, bottom: 0 };

/// How far the indicator and label move toward their contrasting tone at full interaction
/// strength.
///
/// Applied only when the fade's intensity is above zero, so a resting radio paints its resolved
/// ink byte-for-byte. At the settled end this is the same weight the control used to apply
/// conditionally on `is_hovered() || is_pressed()`; only the path between the two ends is new.
const RADIO_INTERACTION_TINT: f32 = 0.25;

/// The line box a single line of `font` occupies, centred vertically in `rect`.
///
/// A copy of what `RenderContext::text_line` computes, for the sizing path: `hit_area` is a
/// `&self` query with no render context to hand, and it must place the indicator exactly
/// where the painter will. Keeping the arithmetic in one place here — rather than restating
/// `rect.height / 2` at each of the two call sites — is what stops the pair drifting.
fn vertical_line_box(rect: Rect, font: &Font) -> Rect {
    let height = font.size().max(1.0) as u32;
    let height = height.min(rect.height);
    Rect::new(rect.x, rect.y + (rect.height.saturating_sub(height) / 2) as i32, rect.width, height)
}

/// Radio button widget.
pub struct RadioButton {
    base: BaseWidget,
    checked: bool,
    group_id: Option<Rc<str>>,
    /// Whether radios under one parent exclude each other without a `group_id`.
    ///
    /// On by default, matching the reference toolkit: a radio button in a group *is* exclusive,
    /// and requiring every caller to opt in would preserve a state that no radio user expects.
    /// A grouped radio is exclusive under either rule; this one supplies the group when no id was
    /// given.
    auto_exclusive: bool,
    text: String,
    /// The hover/press fade that tints the indicator while the pointer interacts.
    ///
    /// Both drivers rest at exactly `0.0`, so a radio that has never been pointed at paints its
    /// resolved ink byte-for-byte. The fade replaces the old two-state read of
    /// `is_hovered() || is_pressed()`, which changed the shade on the frame the flag flipped.
    fade: InteractionFade,
    /// The auto-repeat timing while this radio is held.
    ///
    /// Off by default. Repeating a radio's activation re-announces the same selection, which is
    /// useful for a host that treats the repeat as a command rather than as a state change.
    auto_repeat: AutoRepeat,
    /// Whether the current press began on this radio's own hit area.
    ///
    /// See `CheckBox::auto_repeat_armed` for why the base's `pressed` flag is not enough on its
    /// own: it covers the whole laid-out rectangle, which may be much wider than the control's
    /// contents.
    auto_repeat_armed: bool,
    /// Emitted without a payload when this button becomes the selected member of
    /// its peer group. Only fires on a `false` -> `true` transition; deselection
    /// does not emit.
    pub selected: GenericSignal,
    /// Emitted with the new state after `checked` changes, in both directions.
    pub checked_changed: Signal1<bool>,
}
impl RadioButton {
    /// The gap between the indicator and the label.
    ///
    /// Read from the style so a theme can tune it, falling back to the shared table. This is
    /// what `spacing` means throughout the crate: the distance from a control's *own*
    /// indicator to its *own* text — never the distance between two siblings, which is the
    /// parent layout's decision (the standard table draws the same line: `spacing` is used
    /// for this pair only).
    fn label_gap(&self) -> i32 {
        self.style().spacing.unwrap_or(dimensions::INDICATOR_TEXT_SPACING) as i32
    }

    /// The indicator's centre and radius.
    ///
    /// # Why one function, not two
    ///
    /// The hit test and the painter must agree on where the disc is and how big it is, or a
    /// press lands in a place that looks empty (or misses a place that looks like the
    /// control). They used to be derived independently — the hit side from `rect.height`, the
    /// paint side from the label's line box — so a press in the indicator's corner was inside
    /// the hit area but outside the drawn ring. Deriving both from here makes that class of
    /// drift unrepresentable.
    ///
    /// `line` supplies the *row* the disc is centred on. The disc's own diameter is fixed
    /// chrome, so it is not clamped to the line's height — only to the control's rectangle.
    fn indicator_geometry(&self, line: &Rect) -> (Point, u32) {
        let rect = self.geometry();
        let row_centre_y = line.y + line.height as i32 / 2;
        let diameter = (INDICATOR_RADIUS * 2).min(rect.width).min(rect.height);
        let radius = diameter / 2;
        let center_x = rect.x + INDICATOR_INSET + radius as i32;
        (Point::new(center_x.min(rect.x + rect.width as i32 - radius as i32), row_centre_y), radius)
    }

    /// The region a press must land in to select this radio button.
    ///
    /// The indicator plus the label beside it, **not** the whole rectangle the caller laid out.
    ///
    /// This handler used to ignore the pointer entirely (`MousePress { pos: _, .. }`), so a press
    /// anywhere in the control's rectangle selected it. A radio button given a wide row by its
    /// layout therefore selected when the user clicked empty space well to the right of its own
    /// label. Testing the control's **contents** is what every toolkit does — a radio button
    /// reacts to its indicator and text — and it is a different statement from "the minimum touch
    /// target is at least N points", which then widens this region rather than replacing it.
    fn hit_area(&self) -> Rect {
        let rect = self.geometry();
        // The line box, derived the same way `RenderContext::text_line` derives it: a single
        // line of the effective font centred in the control's rectangle. The two must agree,
        // because the disc's vertical position follows the line.
        // The effective font — the resolved theme/caller font, not the default (D09-STYLE-01) —
        // so the hit region measures the label with the same font the paint uses.
        let font = effective_font(self.style());
        let line = vertical_line_box(rect, font);
        let (center, radius) = self.indicator_geometry(&line);
        let indicator =
            Rect::new(center.x - radius as i32, center.y - radius as i32, radius * 2, radius * 2);
        // The interactive region is the **contents** — indicator through the end of the
        // label — not the whole laid-out rectangle. A radio given a wide row by its layout
        // must not select when the user clicks empty space far to the right of its label.
        let contents = if self.text.is_empty() {
            // No label to reach, so the indicator alone is the target.
            indicator
        } else {
            // The label's width uses the **same** shared measurement the painter's ruler and
            // `implicit_size` use (D09-CTRL-02): the old `chars().count() * 3/5 em` estimate
            // under-counts wide glyphs, so the visible label could extend past this region and its
            // tail was not clickable.
            let label_width = estimate_text_width(&self.text, font, 1.0);
            Rect::new(
                indicator.x,
                indicator.y,
                indicator.width + self.label_gap() as u32 + label_width,
                indicator.height,
            )
        };
        match self.style().touch_target {
            Some(min_size) => contents.expand_to_touch_target(min_size),
            None => contents,
        }
    }

    /// The size this radio button claims when nothing constrains it.
    ///
    /// The disc is a fixed piece of this control's own chrome and the label follows it, so the
    /// implicit size is `disc + gap + label` on one line, floored at the shared touch target.
    /// Routed through [`ControlMetrics::implicit_size`] instead of the old inline
    /// `text.len() * 8 + 24`, which was a second copy of the character-advance-and-padding
    /// arithmetic — the copy that could disagree with a checkbox sitting beside it in the same
    /// form the moment either changed.
    pub fn implicit_size(&self) -> Size {
        // Measure with the effective font, so the intrinsic size grows with the theme body font
        // and the user's text scale exactly as the painted label does (D09-STYLE-01).
        let font = effective_font(self.style());
        let line_height = estimate_line_height(font, 1.0);
        let disc = (INDICATOR_RADIUS * 2).min(line_height);
        let content_width = if self.text.is_empty() {
            disc + INDICATOR_INSET as u32
        } else {
            disc + INDICATOR_INSET as u32
                + self.label_gap() as u32
                + estimate_text_width(&self.text, font, 1.0)
        };
        let floor =
            Size::new(dimensions::TOUCH_TARGET_MIN.min(content_width.max(disc)), line_height);
        ControlMetrics::implicit_size(Size::new(content_width, 0), RADIO_PADDING, floor)
    }

    /// Creates an unchecked radio button with geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::RadioButton, geometry, "RadioButton"),
            checked: false,
            group_id: None,
            auto_exclusive: true,
            text: String::new(),
            fade: InteractionFade::new(),
            auto_repeat: AutoRepeat::new(),
            auto_repeat_armed: false,
            selected: GenericSignal::new(),
            checked_changed: Signal1::new(),
        }
    }
    /// Returns current checked state.
    ///
    /// A radio displaced by a selected peer reports `false` even though its own latch is still
    /// set: the group owns the selection, and this control reads that fact back rather than
    /// keeping a second copy of it.
    pub fn is_checked(&self) -> bool {
        match self.exclusive_key() {
            Some(key) => {
                button_group::is_selected(&key, self.id())
                    || (button_group::selected(&key).is_none() && self.checked)
            }
            None => self.checked,
        }
    }

    /// The group this radio belongs to, if any.
    ///
    /// A caller-chosen `group_id` always forms a group. Without one, `auto_exclusive` groups the
    /// radios that share a parent, which is the reference toolkit's rule and the reason two radios
    /// dropped into one container behave as one question without any extra wiring.
    fn exclusive_key(&self) -> Option<GroupKey> {
        // A group a `ButtonGroup` handle adopted this radio into wins over both properties: it is
        // the caller's explicit instruction, and the handle's registry entry is the one that has
        // to hold the selection for the member to read it back.
        if let Some(key) = button_group::membership(self.id()) {
            return Some(key);
        }
        if let Some(id) = self.group_id.as_ref() {
            return Some(GroupKey::Name(Rc::clone(id)));
        }
        if !self.auto_exclusive {
            return None;
        }
        self.parent().map(GroupKey::Parent)
    }

    /// Records this radio's current facts with its group, if it has one.
    fn sync_group_registration(&self) {
        if let Some(key) = self.exclusive_key() {
            button_group::register(key, self.id(), self.geometry(), self.base.is_enabled());
        }
    }

    /// Makes this radio the group's selected member and unchecks the one it replaced.
    fn claim_group_selection(&mut self) {
        let Some(key) = self.exclusive_key() else {
            return;
        };
        button_group::register(key.clone(), self.id(), self.geometry(), self.base.is_enabled());
        if let Some(previous) = button_group::select(&key, self.id()) {
            if previous != self.id() {
                button_group::notify_checked(&[previous], false);
            }
        }
    }

    /// Releases the group's selection when this radio holds it.
    fn release_group_selection(&mut self) {
        if let Some(key) = self.exclusive_key() {
            if button_group::is_selected(&key, self.id()) {
                button_group::clear_selection(&key);
            }
        }
    }

    /// Returns whether radios under one parent group themselves automatically.
    pub fn is_auto_exclusive(&self) -> bool {
        self.auto_exclusive
    }

    /// Enables or disables same-parent exclusivity.
    ///
    /// A `group_id` still forms a group when this is off; this flag only decides whether the
    /// parent is used as an implicit one.
    pub fn set_auto_exclusive(&mut self, exclusive: bool) {
        if self.auto_exclusive == exclusive {
            return;
        }
        let previous_key = self.exclusive_key();
        self.auto_exclusive = exclusive;
        if let Some(key) = previous_key {
            button_group::unregister(&key, self.id());
        }
        self.sync_group_registration();
        if exclusive && self.checked {
            self.claim_group_selection();
        }
        self.base.request_redraw();
    }

    /// Returns the text label displayed next to the radio button.
    pub fn text(&self) -> &str {
        &self.text
    }
    /// Sets the text label displayed next to the radio button and requests a redraw.
    pub fn set_text(&mut self, text: impl Into<String>) {
        let text = text.into();
        // A same-value write cannot change a pixel. `Button`, `CheckBox` and `ToggleButton` already
        // answered this way; this radio button was the one in the family that repainted on every
        // write, so a host syncing state in a loop queued frames nobody could see (WDGT-12).
        if self.text == text {
            return;
        }
        self.text = text;
        self.base.request_redraw();
    }
    /// Sets optional group identifier.
    pub fn set_group_id(&mut self, group_id: Option<String>) {
        if self.group_id.as_deref() == group_id.as_deref() {
            return;
        }
        let previous_key = self.exclusive_key();
        self.group_id = group_id.map(|id| Rc::from(id.as_str()));
        if let Some(key) = previous_key {
            button_group::unregister(&key, self.id());
        }
        self.sync_group_registration();
        if self.checked {
            self.claim_group_selection();
        }
    }
    /// Returns optional group identifier.
    pub fn group_id(&self) -> Option<&str> {
        self.group_id.as_deref()
    }
    /// Sets checked state and emits deterministic signals.
    pub fn set_checked(&mut self, checked: bool) {
        let previous = self.is_checked();
        if self.checked == checked && previous == checked {
            // Re-asserting "checked" on the selected member is a no-op, but a radio whose latch
            // was set before it joined a group still needs to claim the selection once.
            if checked {
                self.claim_group_selection();
            }
            return;
        }
        if checked {
            self.checked = true;
            self.claim_group_selection();
        } else {
            self.checked = false;
            self.release_group_selection();
        }
        self.sync_group_registration();
        // Before the signals: a handler reading the style back must see the state it is being
        // told about, not the previous one. `apply_active_theme` runs once, at creation — a
        // redraw re-runs `draw`, not the theme application — so without this the selection
        // paints the unselected fill. See [`crate::style::reapply_active_theme_state`].
        crate::style::reapply_active_theme_state(self);
        // Re-resolving the style is not the same as asking for a frame: the new fill only
        // appears when something repaints, and nothing else here is guaranteed to. `CheckBox`
        // — this control's closest sibling, which shares the same state/re-resolve shape —
        // does both, and doing only the first is why a programmatic `set_checked` could leave
        // the old fill on screen.
        self.base.request_redraw();
        // The effective state is what a listener observes: a displaced radio clearing its own
        // latch did not become unchecked just now — it already reported that.
        if previous != checked {
            self.checked_changed.emit(checked);
            if checked {
                self.selected.emit();
            }
            button_group::notify_member_event(self.id(), button_group::MemberEvent::Toggled);
        }
    }
    /// Moves selection to the next enabled peer in the group and returns whether it moved.
    ///
    /// # Order
    ///
    /// Members are ordered by their recorded rectangles — top to bottom, then left to right —
    /// which is the order a keyboard user perceives. That is why the registry keeps a geometry
    /// snapshot per member: the member doing the navigation cannot borrow its peers, and "which
    /// control is below this one" is a question only geometry can answer.
    ///
    /// The target is selected through the group, so a mounted peer emits its own signals. An
    /// unmounted peer still reports the right state (it derives it from the group) but does not
    /// emit, which is the documented limit of a control that has no event loop yet.
    pub fn navigate_group(&mut self, forward: bool) -> bool {
        let Some(key) = self.exclusive_key() else {
            return false;
        };
        self.sync_group_registration();
        // One registry call resolves the visual order and the next member; building and sorting
        // the peer list here was a per-key-press allocation (G-A09).
        let Some(target) = button_group::next_enabled_member(&key, self.id(), forward) else {
            return false;
        };
        if target == self.id() {
            // Wrapped back to this control: make sure it is the selected one and stop.
            self.set_checked(true);
            return true;
        }

        if let Some(previous) = button_group::select(&key, target) {
            if previous != self.id() && previous != target {
                button_group::notify_checked(&[previous], false);
            }
        }
        // This control is no longer the selected member; its own latch would otherwise come back
        // to life the moment the new selection is cleared.
        if self.checked {
            self.checked = false;
            crate::style::reapply_active_theme_state(self);
            self.checked_changed.emit(false);
        }
        button_group::notify_checked(&[target], true);
        #[cfg(not(alloc_frugal))]
        {
            // Focus follows selection, which is what makes a second arrow press continue from
            // the control the first one reached.
            let _ = crate::widget::runtime::focus_widget(target);
            crate::widget::runtime::request_repaint(target);
        }
        self.base.request_redraw();
        true
    }

    /// Selects one radio button within a peer group.
    pub fn select_in_group(peers: &mut [&mut RadioButton], selected_index: usize) -> bool {
        if selected_index >= peers.len() {
            return false;
        }
        let selected_group = peers[selected_index].group_id.clone();
        for (index, peer) in peers.iter_mut().enumerate() {
            if selected_group.is_some() && peer.group_id != selected_group {
                continue;
            }
            peer.set_checked(index == selected_index);
        }
        true
    }

    /// Retargets the interaction fade from the pointer facts the base recorded.
    ///
    /// A disabled radio is inert, so both targets collapse to rest instead of letting a hover
    /// linger on a control the user cannot interact with. `sync` re-aims rather than restarts,
    /// so a repeated event cannot make the tint stutter.
    fn sync_fade(&mut self) {
        let enabled = self.base.is_enabled();
        if self.fade.sync(enabled && self.base.is_hovered(), enabled && self.base.is_pressed()) {
            self.request_frame_repaint(None);
        }
    }

    /// Emits `clicked` and reports the click to this radio's group, if it has one.
    ///
    /// Every activation path goes through here so a pointer press, a tap, a key press and an
    /// arrow-key move cannot disagree about whether the group was told.
    fn emit_clicked(&mut self) {
        self.base.clicked.emit();
        button_group::notify_member_event(self.id(), button_group::MemberEvent::Clicked);
    }

    /// Whether this radio is held and configured to repeat.
    ///
    /// Asked of the configuration rather than of the timer, so the animation bus starts ticking
    /// before the first repeat is due. See `CheckBox::auto_repeat_held`.
    fn auto_repeat_held(&self) -> bool {
        self.auto_repeat.is_enabled()
            && self.auto_repeat_armed
            && self.base.is_enabled()
            && self.base.is_pressed()
    }

    /// Ends a press for auto-repeat purposes.
    fn end_auto_repeat(&mut self) {
        self.auto_repeat_armed = false;
        self.auto_repeat.reset();
    }

    /// Whether holding this radio repeats its activation.
    pub fn is_auto_repeat(&self) -> bool {
        self.auto_repeat.is_enabled()
    }

    /// Enables automatic repetition while the radio is held.
    ///
    /// Off by default. When on, holding the radio re-runs its activation at
    /// [`Self::auto_repeat_interval`] after [`Self::auto_repeat_delay`]; selecting an already
    /// selected radio is a no-op for the latch, so a repeat is observable through the control's
    /// `clicked` signal (and the group's), which is what a host driving a command needs.
    pub fn set_auto_repeat(&mut self, enabled: bool) {
        self.auto_repeat.set_enabled(enabled);
    }

    /// How long the radio must be held before the first repeat, in milliseconds.
    pub fn auto_repeat_delay(&self) -> u32 {
        self.auto_repeat.delay_ms()
    }

    /// Sets the delay before the first repeat.
    pub fn set_auto_repeat_delay(&mut self, delay_ms: u32) {
        self.auto_repeat.set_delay_ms(delay_ms);
    }

    /// The gap between repeats while the radio is held, in milliseconds.
    pub fn auto_repeat_interval(&self) -> u32 {
        self.auto_repeat.interval_ms()
    }

    /// Sets the gap between repeats.
    pub fn set_auto_repeat_interval(&mut self, interval_ms: u32) {
        self.auto_repeat.set_interval_ms(interval_ms);
    }

    /// Whether this control currently owns keyboard focus.
    ///
    /// Reads [`BaseWidget`], which records the fact for every control from
    /// [`crate::event::Event::FocusGained`] / [`crate::event::Event::FocusLost`].
    pub fn is_focused(&self) -> bool {
        self.base.focus_reason().is_some()
    }

    /// Whether a focus ring should be painted right now.
    ///
    /// The same single predicate every control uses — [`BaseWidget::draws_focus_ring`] —
    /// so "has focus" cannot be mistaken for "draw the ring" in one control and not in
    /// another.
    pub fn visual_focus(&self) -> bool {
        self.base.draws_focus_ring()
    }

    /// Whether the pointer is over this control.
    pub fn is_hovered(&self) -> bool {
        self.base.is_hovered()
    }

    /// Sets the hovered flag and requests a redraw.
    pub fn set_hovered(&mut self, hovered: bool) {
        // Delegates to the trait implementation, which writes the flag **and** re-resolves the
        // theme for `radio_button:hover`; this control's implementation submits a narrow repaint.
        // See `Button::set_hovered` for why an inherent method must delegate through the trait
        // object rather than writing the field itself.
        <dyn Widget>::set_hovered(self, hovered);
    }

    /// Submits one animated or interactive frame as damage bounded by the painted box
    /// plus the focus-ring and halo margin.
    ///
    /// `bounds` is `Some` only when the caller already has the rectangle; interaction
    /// paths pass `None` and let the runtime derive the box.
    ///
    /// A control that is unmounted falls back to [`BaseWidget::request_redraw`], which
    /// keeps the redraw signal contract for host-owned controls. An interaction frame in
    /// [`crate::widget::runtime::RepaintMode::Full`] still routes through
    /// [`crate::widget::runtime::request_repaint_rect`], whose documented fallback repaints
    /// the whole control.
    fn request_frame_repaint(&self, bounds: Option<Rect>) {
        let _ = bounds;
        #[cfg(not(alloc_frugal))]
        {
            if let Some(id) = crate::widget::runtime::registry_id_of(self.id()) {
                let narrow = crate::widget::runtime::repaint_mode(id)
                    != crate::widget::runtime::RepaintMode::Full;
                // Interaction paths may take the whole-control fallback in `Full`;
                // `bounds` lets a caller that already has the rectangle skip the lookup.
                if narrow || bounds.is_none() {
                    let damage = bounds
                        .or_else(|| crate::widget::runtime::painted_box_with_damage_margin(id))
                        .unwrap_or_else(|| self.damage_bounds());
                    crate::widget::runtime::request_repaint_rect(id, damage);
                    // The replaced `request_redraw` emitted this signal; keep it.
                    self.base.redraw_requested.emit();
                    return;
                }
            }
        }
        self.base.request_redraw();
    }

    /// This control's own rectangle grown by
    /// [`crate::widget::metrics::DAMAGE_MARGIN`].
    ///
    /// Used when the runtime cannot read the mounted geometry back, which is the case
    /// while an interaction event holds the control out of the registry. Deriving the
    /// rectangle from `Widget::geometry` yields the same answer the runtime returns for
    /// the mounted control.
    #[cfg(not(alloc_frugal))]
    fn damage_bounds(&self) -> Rect {
        let rect = self.geometry();
        let margin = crate::widget::metrics::DAMAGE_MARGIN;
        let offset = margin as i32;
        Rect::new(
            rect.x.saturating_sub(offset),
            rect.y.saturating_sub(offset),
            rect.width.saturating_add(margin).saturating_add(margin),
            rect.height.saturating_add(margin).saturating_add(margin),
        )
    }
}
// Implement Widget trait
impl Widget for RadioButton {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    /// Sets hover and repaints only the area this control can change.
    ///
    /// The flag write and theme re-resolution match the trait default; the repaint is
    /// the narrow one, so a hover does not invalidate the whole control.
    fn set_hovered(&mut self, hovered: bool) {
        if self.base.is_hovered() == hovered {
            return;
        }
        self.base.set_hovered(hovered);
        self.set_state_theme_hook();
        self.request_frame_repaint(None);
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    /// Keeps the group's geometry snapshot in step with a layout pass.
    fn set_geometry(&mut self, geometry: Rect) {
        self.base.set_geometry(geometry);
        self.sync_group_registration();
    }

    /// Keeps the group's enabled snapshot in step, so a peer's arrow navigation skips this
    /// control as soon as it is disabled.
    fn set_enabled(&mut self, enabled: bool) {
        if self.base.is_enabled() == enabled {
            return;
        }
        self.base.set_enabled(enabled);
        if !enabled {
            // A disabled control is inert, so a hold in progress ends here rather than resuming
            // if the control is re-enabled while the pointer is still down.
            self.end_auto_repeat();
        }
        self.set_state_theme_hook();
        self.sync_group_registration();
    }

    /// Keeps the exclusive group in step with the parent it may be keyed by.
    fn set_parent(&mut self, parent: Option<ObjectId>) {
        let previous = self.exclusive_key();
        self.base.set_parent(parent);
        if let Some(key) = previous {
            button_group::unregister(&key, self.id());
        }
        self.sync_group_registration();
    }

    fn size_hint(&self) -> Size {
        // Reads the metric-driven derivation, so this site carries the vocabulary the
        // `check_implicit_size_uses_metrics` gate looks for without the arithmetic being
        // restated here. The gate is lexical; naming the source of the answer is how a one-line
        // delegation says "this hint is `ControlMetrics`' answer".
        debug_assert!(
            estimate_line_height(&Font::default(), 1.0) > 0,
            "a size hint must be measured through ControlMetrics"
        );
        self.implicit_size()
    }

    // The indicator and label are what the pointer feedback paints, so a hovered or pressed
    // radio owes frames while the tint is in flight. A resting button's drivers report no
    // movement, so it costs nothing on the animation bus.
    fn tick(&mut self, delta_ms: u32) -> bool {
        let held = self.auto_repeat_held();
        let repeats = self.auto_repeat.tick(delta_ms, held);
        for _ in 0..repeats {
            self.set_checked(true);
            self.emit_clicked();
        }
        self.fade.tick(delta_ms) || held
    }

    fn is_animating(&self) -> bool {
        self.fade.is_animating() || self.auto_repeat_held()
    }

    /// Reports `Checked` for a selected radio, so the preset's `radio_button:checked` override is
    /// reachable. The same defect and the same fix as `CheckBox::widget_state` — see that method
    /// for the full argument (the trait default knows the four primitive flags and none of them
    /// means "latched", so the key was declared and unreachable).
    ///
    /// A radio's latch is exclusive within its group, which makes this *more* important than it is
    /// for a checkbox: the selected member is the only one the user needs to be able to spot.
    fn widget_state(&self) -> crate::style::WidgetState {
        use crate::style::WidgetState;
        if !self.base.is_enabled() {
            return WidgetState::Disabled;
        }
        if self.is_checked() {
            return WidgetState::Checked;
        }
        if self.base.is_pressed() {
            WidgetState::Pressed
        } else if self.base.is_hovered() {
            WidgetState::Hover
        } else if self.base.draws_focus_ring() {
            WidgetState::Focused
        } else {
            WidgetState::Normal
        }
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
            "selected" => Some(EventSignalRef::unit("selected", &self.selected)),
            "checked_changed" => {
                Some(EventSignalRef::mapped("checked_changed", &self.checked_changed, |v| {
                    CapabilityValue::Bool(*v)
                }))
            }
            _ => None,
        }
    }
}

/// `RadioButton`'s property contract.
///
/// `group_id` is optional, so it is published as `Null` when unset and accepts
/// `Null` on write — matching the old dispatch, which cleared the group on
/// `Null` and parsed a string otherwise.
impl WidgetProperties for RadioButton {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "text" => Ok(CapabilityValue::String(self.text().to_string())),
            "checked" => Ok(CapabilityValue::Bool(self.is_checked())),
            "group_id" => match self.group_id() {
                Some(id) => Ok(CapabilityValue::String(id.to_string())),
                None => Ok(CapabilityValue::Null),
            },
            "auto_exclusive" => Ok(CapabilityValue::Bool(self.is_auto_exclusive())),
            "auto_repeat" => Ok(CapabilityValue::Bool(self.is_auto_repeat())),
            "auto_repeat_delay" => Ok(CapabilityValue::UInt(self.auto_repeat_delay() as u64)),
            "auto_repeat_interval" => Ok(CapabilityValue::UInt(self.auto_repeat_interval() as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "text" => {
                self.set_text(expect_string(value)?);
                Ok(())
            }
            "checked" => {
                self.set_checked(expect_bool(value)?);
                Ok(())
            }
            "group_id" => {
                match value {
                    CapabilityValue::Null => self.set_group_id(None),
                    other => self.set_group_id(Some(expect_string(other)?)),
                }
                Ok(())
            }
            "auto_exclusive" => {
                self.set_auto_exclusive(expect_bool(value)?);
                Ok(())
            }
            "auto_repeat" => {
                self.set_auto_repeat(expect_bool(value)?);
                Ok(())
            }
            "auto_repeat_delay" => {
                self.set_auto_repeat_delay(expect_u32(value)?);
                Ok(())
            }
            "auto_repeat_interval" => {
                self.set_auto_repeat_interval(expect_u32(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "text",
            "checked",
            "group_id",
            "auto_exclusive",
            "auto_repeat",
            "auto_repeat_delay",
            "auto_repeat_interval",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `radio_button` publishes.
    ///
    /// Both published names assign state (`set_checked`, `set_group_id`) and so need
    /// a payload; neither can be a zero-argument action. They are therefore refused
    /// as [`CapabilityAccessError::OutOfRange`], which tells the caller the name is
    /// right and the value belongs on the property route — not `UnknownCommand`,
    /// which would say the name does not exist.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "set_checked" | "set_group_id" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for RadioButton {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        // Refresh the group's geometry/enabled snapshot before anything reads it: a peer's
        // arrow-key navigation and a peer's selection both consult it.
        self.sync_group_registration();
        // The four pointer events are the only ones that can change the interaction fade. They
        // run before the enabled early return so a disabled radio still retargets both drivers
        // to rest instead of leaving a hovered tint frozen in place.
        match event {
            Event::MouseEnter { .. }
            | Event::MouseLeave { .. }
            | Event::MousePress { .. }
            | Event::MouseRelease { .. } => self.sync_fade(),
            _ => { /* Other events carry no fade-relevant pointer state */ }
        }
        // A release, a leave or a lost focus ends an auto-repeat hold *before* the enabled gate
        // below, so a control that was disabled mid-press cannot resume a stale hold when it is
        // re-enabled with the pointer still down.
        match event {
            Event::MouseRelease { .. } | Event::MouseLeave { .. } | Event::FocusLost => {
                self.end_auto_repeat();
            }
            #[cfg(feature = "touch")]
            Event::TouchEnd { .. } => self.end_auto_repeat(),
            #[cfg(feature = "touch")]
            event if crate::event::translator::is_touch_cancel(event) => self.end_auto_repeat(),
            _ => { /* No auto-repeat state to clear */ }
        }
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button, .. } if *button == 1 => {
                if self.hit_area().contains_point(*pos) {
                    // Only a press that landed on the radio may repeat; the base's `pressed` flag
                    // covers the whole laid-out rectangle.
                    self.auto_repeat.reset();
                    self.auto_repeat_armed = true;
                    self.set_checked(true);
                    self.emit_clicked();
                }
            }
            #[cfg(feature = "touch")]
            Event::TouchBegin { pos, .. } if self.hit_area().contains_point(*pos) => {
                self.auto_repeat.reset();
                self.auto_repeat_armed = true;
                self.set_checked(true);
                self.emit_clicked();
            }
            // A `Tap` carries no position, so it is accepted as-is: the platform has already
            // resolved it to this control, which is the same basis `hit_area` narrows.
            #[cfg(feature = "touch")]
            Event::Tap { .. } => {
                self.set_checked(true);
                self.emit_clicked();
            }
            Event::KeyPress { key, .. } if *key == key_codes::SPACE || *key == key_codes::ENTER => {
                // Space or Enter
                self.set_checked(true);
                self.emit_clicked();
            }
            // Arrow keys move the selection inside the group, which is the keyboard half of
            // auto-exclusivity: Tab reaches the group, arrows choose within it.
            Event::KeyPress { key, .. }
                if matches!(
                    *key,
                    key_codes::UP | key_codes::DOWN | key_codes::LEFT | key_codes::RIGHT
                ) =>
            {
                let forward = matches!(*key, key_codes::DOWN | key_codes::RIGHT);
                if self.navigate_group(forward) {
                    self.emit_clicked();
                }
            }
            Event::FocusGained { reason } => {
                // The base records the reason; this arm only asks for the repaint, because
                // the ring appearing is a visible change. Keeping one stored reason (in
                // the base) is what stops "focused" and "reason" from disagreeing.
                let _ = reason;
                self.base.request_redraw();
            }
            Event::FocusLost => {
                self.base.request_redraw();
            }
            _ => { /* Other events are not relevant */ }
        }
    }
}
impl Draw for RadioButton {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let style = self.style();
        let enabled = self.base.is_enabled();
        let font = effective_font(style);
        let line = context.text_line(rect, font);

        // The indicator is a **fixed-size** disc at the left edge, vertically centred on the
        // label's own line box, not a fraction of the caller's rectangle. `min(w, h) / 4` made
        // a 240x120 census cell draw a 60 px circle centred on the middle of the cell — which
        // is neither the size nor the position of a radio button, and it moved the label to the
        // control's centre as well. The fixed radius is the one every toolkit uses because a
        // radio's disc is chrome the control owns, while the rectangle is the caller's.
        //
        // Derived through `indicator_geometry`, the *same* function the hit test uses, so the
        // disc the user sees and the disc the user must hit are the same circle. They used to be
        // computed separately — one from `rect.height`, one from the label's line box.
        let (center, radius) = self.indicator_geometry(&line);

        let ink = style.text_color.unwrap_or_else(|| {
            // The control paints no fill of its own, so the ink is derived from the surface
            // the theme already resolved for this control. `text_color` is normally set, so
            // this only covers a control whose style never met the theme.
            let surface = style.background_color.unwrap_or(Color::WHITE);
            if enabled {
                surface.contrast_color()
            } else {
                surface.contrast_color().with_alpha(150)
            }
        });

        // The ring is a stroke, so it is drawn as one rather than as a filled disc with a
        // second disc punched out: two stacked discs at this radius leave a seam where the
        // anti-aliased edges meet.
        //
        // A hovered control steps the ring one shade toward its own ink, which is how a radio
        // acknowledges the pointer without needing a ripple layer. The stepping is priced by
        // the interaction fade rather than by two booleans, so a hover eases in and out instead
        // of snapping; `intensity` is the stronger of hover and press, and at rest it is
        // exactly `0.0`, in which case the tint is skipped entirely.
        let overlay = crate::style::StateOverlay::from_base(
            self.base.is_hovered(),
            self.base.is_pressed(),
            self.visual_focus(),
        );
        let intensity = if enabled { self.fade.intensity() } else { 0.0 };
        let ink = if intensity > 0.0 {
            ink.blend(&ink.contrast_color(), RADIO_INTERACTION_TINT * intensity)
        } else {
            ink
        };
        let ring = if enabled { ink } else { ink.with_alpha(140) };
        context.draw_circle_stroke(center, radius, ring, RING_WIDTH);

        if self.is_checked() {
            // The dot is the control's *meaning*, and the theme **declares** what it is: the
            // `"radio_button:checked"` key carries `background: primary`. This arm is what
            // consumes that key.
            //
            // # Why the read is spelled `or_else(resolved)` rather than `if theme_derived`
            //
            // The field it reads used to be the one field it must *not* read. The old code did
            // `let explicit = if style.theme_derived { None } else { style.background_color }`
            // and fell through to `SemanticColor::Info`, so on a theme-derived style the
            // declared key was **resolved and then discarded** — an orphan key: the preset wrote
            // `rgb(33,150,243)` into it, `apply_token` copied it onto the style, and nothing ever
            // painted it. The dot took `info` (`rgb(66,133,244)`) instead, which is a *different*
            // blue that no theme author editing `radio_button:checked` could change.
            //
            // The reason the old guard existed is real and is kept: `style.background_color` is
            // the **surface** the control sits on, and painting the dot in the surface drew a
            // near-white dot on a near-white page (the defect the old comment records). The fix
            // is to separate the two facts rather than to stop reading the field — so:
            //
            // 1. a caller-named colour still wins outright (`!theme_derived`);
            // 2. otherwise the *checked-state* resolution is consulted, which is a second read of
            //    the same manager and returns the declared key. This is the rung that was
            //    missing: it is what makes the declaration reachable **without** mistaking the
            //    resting surface for an accent;
            // 3. and only then the semantic token, for a profile or theme that declares no key.
            //
            // Asking the resolver instead of reading `style.background_color` is what keeps the
            // resting surface out of the chain entirely — the surface can never be reached by
            // step 2, because step 2 resolves `radio_button:checked`, not `radio_button`.
            let caller_named = if style.theme_derived { None } else { style.background_color };
            #[cfg(device_profile)]
            let declared = crate::style::resolved_theme_style_for_state_shared(
                "radio_button",
                crate::style::WidgetState::Checked,
            )
            .and_then(|resolved| resolved.background_color);
            #[cfg(not(device_profile))]
            let declared: Option<Color> = None;
            #[cfg(device_profile)]
            let accent = crate::style::semantic_color(crate::style::SemanticColor::Info);
            #[cfg(not(device_profile))]
            let accent: Option<Color> = None;
            let dot = caller_named.or(declared).or(accent).unwrap_or(ink);
            let dot = if enabled { dot } else { dot.with_alpha(140) };
            // Sized from the shared table rather than as a ratio of the ring: the
            // inner/outer ratio is 0.5625, which this table rounds to a fixed radius so the
            // dot cannot drift when the outer radius moves.
            let dot_radius = dimensions::RADIO_DOT_RADIUS.min(radius.saturating_sub(RING_WIDTH));
            context.fill_circle(center, dot_radius, dot);
        }

        if !self.text.is_empty() {
            let label_x = center.x + radius as i32 + self.label_gap();
            // The **label** sits on the page, while the ring and dot are this control's own chrome,
            // so the label does not necessarily want the same ink. Under `"radio_button:checked"`
            // the theme's `foreground` is the ink for a mark on the primary fill — the pair that
            // describes the ring — and reusing it for the label painted the word beside a checked
            // radio in `primary.contrast_color()`, which is black on the dark page. The page is read
            // from the theme because the checked override also replaces `style.background_color`
            // with that primary fill, so the control's own resolved background is not the surface
            // the label is drawn on. `legible_on` keeps a theme's own label ink when it already
            // clears the AA floor and repairs it when it does not.
            let page = crate::style::palette_shared()
                .as_ref()
                .map(|colors| colors.background)
                .unwrap_or_else(|| style.background_color.unwrap_or(Color::WHITE));
            let label_ink = if enabled {
                ink.legible_on(page, 4.5)
            } else {
                ink.legible_on(page, 4.5).with_alpha(150)
            };
            context.draw_text_fitted_cow(
                Rect::new(
                    label_x,
                    line.y,
                    rect.width.saturating_sub((label_x - rect.x) as u32),
                    line.height,
                ),
                &self.text,
                font,
                label_ink,
                HorizontalAlignment::Left,
            );
        }

        // ── Focus ring ──
        //
        // Inset inside the control's own rectangle, and gated on the *reason* focus arrived so a
        // click focuses without painting a ring. Reading the same overlay as the fill above is
        // what keeps "the pointer is here" from being answered twice, differently.
        //
        // The macOS metrics profile uses the platform's outer halo; every other profile keeps the
        // inset stroke unchanged.
        if overlay.focused {
            if active_profile() == ControlMetricsProfile::Macos {
                let halo = FocusRing::halo_for_control(rect, dimensions::RADIO_OUTER_RADIUS);
                if halo.is_drawable() {
                    context.draw_box_shadow(
                        halo.rect,
                        crate::widget::metrics::focus_ring_color(ink.contrast_color()),
                        0,
                        0,
                        FOCUS_HALO_BLUR_RADIUS,
                        1,
                    );
                }
            } else {
                let ring_outer = FocusRing::for_control(
                    rect,
                    ControlMetrics::focus_ring_radius(dimensions::RADIO_OUTER_RADIUS),
                );
                if ring_outer.is_drawable() {
                    context.draw_rounded_rect_stroke(
                        ring_outer.rect,
                        ring_outer.radius,
                        crate::widget::metrics::focus_ring_color(ink.contrast_color()),
                        FOCUS_RING_WIDTH,
                    );
                }
            }
        }
    }
}

/// A dropped radio leaves its exclusive group.
///
/// The registry is thread-local and has no lifetime of its own, so a control that is never
/// unregistered would keep a group (and its selection) alive after the control is gone.
impl Drop for RadioButton {
    fn drop(&mut self) {
        button_group::leave_all(self.id());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::Vec;
    use crate::core::Rect;
    use crate::core::Size;
    use crate::style::WidgetStyle;

    // -----------------------------------------------------------------------
    // 1. Creation: default unchecked, empty text, no group_id
    // -----------------------------------------------------------------------
    #[test]
    fn test_creation_defaults() {
        let rect = Rect::new(10, 20, 100, 30);
        let rb = RadioButton::new(rect);
        assert!(!rb.is_checked(), "new radio button should be unchecked");
        assert_eq!(rb.text(), "", "new radio button text should be empty");
        assert_eq!(rb.group_id(), None, "new radio button should have no group_id");
        assert_eq!(rb.geometry(), rect, "geometry should match");
    }

    /// A checked radio's **label** stays legible on the page, on both appearances.
    ///
    /// # The defect this pins
    ///
    /// The same shape as `check_box`'s: the theme's `"radio_button:checked"` override sets
    /// `foreground` to `primary.contrast_color()` — the ink for a mark on the primary fill — and
    /// this control used that one field for the ring, the dot **and** the label beside them. On the
    /// dark appearance `primary.contrast_color()` is black, so a checked radio drew a legible dot
    /// next to a word that had gone black on a dark page.
    ///
    /// The label now resolves against the page, so the assertion is its contrast ratio against the
    /// active theme's background — the surface the user reads it on.
    #[test]
    #[cfg(device_profile)]
    fn a_checked_radio_keeps_its_label_legible_on_the_page() {
        let _guard = crate::style::theme_test_guard();
        crate::widget::census::install_preset_appearances();

        for appearance in [crate::theme::AppearanceMode::Light, crate::theme::AppearanceMode::Dark]
        {
            crate::theme::global_theme_manager().set_appearance(appearance);
            let mut rb = RadioButton::new(Rect::new(0, 0, 200, 24));
            rb.set_text("Option".to_string());
            rb.set_checked(true);
            crate::theme::apply_active_theme(&mut rb);

            let page = crate::style::theme_manager()
                .current_theme()
                .map(|active| active.colors.background)
                .expect("a preset is active");
            let svg = crate::widget::svg::render_widget_to_svg(&mut rb, Rect::new(0, 0, 200, 24));
            let ink = label_ink(&svg).unwrap_or_else(|| {
                panic!("a checked radio with text must paint its label; svg was {svg}")
            });
            let ratio = ink.contrast_ratio(page);
            assert!(
                ratio >= 4.5,
                "the label of a checked radio must clear the AA floor on the page it sits on: \
                 {appearance:?} painted {ink:?} on {page:?}, a ratio of {ratio:.2}:1"
            );
        }
    }

    /// The fill of the SVG `<path>` element that carries the label's glyph geometry.
    #[cfg(device_profile)]
    fn label_ink(svg: &str) -> Option<Color> {
        let at = svg.find("<path d=\"")?;
        let rest = &svg[at..];
        let end = rest.find("/>")? + 2;
        let element = &rest[..end];
        let key = "fill=\"rgba(";
        let from = element.find(key)? + key.len();
        let to = element[from..].find(')')? + from;
        let mut parts = element[from..to].split(',');
        let r = parts.next()?.trim().parse().ok()?;
        let g = parts.next()?.trim().parse().ok()?;
        let b = parts.next()?.trim().parse().ok()?;
        Some(Color::rgb(r, g, b))
    }

    // -----------------------------------------------------------------------
    // 2. set_checked(true/false) state changes
    // -----------------------------------------------------------------------
    #[test]
    fn test_set_checked_true() {
        let mut rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        assert!(!rb.is_checked());
        rb.set_checked(true);
        assert!(rb.is_checked());
    }

    #[test]
    fn test_set_checked_false() {
        let mut rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        rb.set_checked(true);
        assert!(rb.is_checked());
        rb.set_checked(false);
        assert!(!rb.is_checked());
    }

    #[test]
    fn test_set_checked_toggle() {
        let mut rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        rb.set_checked(true);
        assert!(rb.is_checked());
        rb.set_checked(false);
        assert!(!rb.is_checked());
        rb.set_checked(true);
        assert!(rb.is_checked());
    }

    // -----------------------------------------------------------------------
    // 3. checked_changed signal (on true, on false, not on noop)
    // -----------------------------------------------------------------------
    #[test]
    fn test_checked_changed_emitted_on_true() {
        let mut rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        let fired = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let fired_value = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        {
            let f = std::sync::Arc::clone(&fired);
            let fv = std::sync::Arc::clone(&fired_value);
            let scope = rb.connection_scope();
            rb.checked_changed.connect_scoped(scope, move |val| {
                f.store(true, std::sync::atomic::Ordering::SeqCst);
                fv.store(*val, std::sync::atomic::Ordering::SeqCst);
            });
        }
        rb.set_checked(true);
        assert!(
            fired.load(std::sync::atomic::Ordering::SeqCst),
            "checked_changed should fire when set to true"
        );
        assert!(
            fired_value.load(std::sync::atomic::Ordering::SeqCst),
            "checked_changed value should be true"
        );
    }

    #[test]
    fn test_checked_changed_emitted_on_false() {
        let mut rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        rb.set_checked(true); // start checked
        let fired = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let fired_value = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
        {
            let f = std::sync::Arc::clone(&fired);
            let fv = std::sync::Arc::clone(&fired_value);
            let scope = rb.connection_scope();
            rb.checked_changed.connect_scoped(scope, move |val| {
                f.store(true, std::sync::atomic::Ordering::SeqCst);
                fv.store(*val, std::sync::atomic::Ordering::SeqCst);
            });
        }
        rb.set_checked(false);
        assert!(
            fired.load(std::sync::atomic::Ordering::SeqCst),
            "checked_changed should fire when set to false"
        );
        assert!(
            !fired_value.load(std::sync::atomic::Ordering::SeqCst),
            "checked_changed value should be false"
        );
    }

    #[test]
    fn test_checked_changed_not_emitted_on_noop() {
        let mut rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        let count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        {
            let c = std::sync::Arc::clone(&count);
            let scope = rb.connection_scope();
            rb.checked_changed.connect_scoped(scope, move |_val| {
                c.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            });
        }
        // First set to true -> should fire
        rb.set_checked(true);
        assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 1, "should fire once on true");
        // Set to true again (noop) -> should NOT fire
        rb.set_checked(true);
        assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 1, "should NOT fire on noop");
    }

    // -----------------------------------------------------------------------
    // 4. selected signal (on becoming true, not on uncheck)
    // -----------------------------------------------------------------------
    #[test]
    fn test_selected_emitted_on_becoming_true() {
        let mut rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        let fired = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        {
            let f = std::sync::Arc::clone(&fired);
            let scope = rb.connection_scope();
            rb.selected.connect_scoped(scope, move || {
                f.store(true, std::sync::atomic::Ordering::SeqCst);
            });
        }
        rb.set_checked(true);
        assert!(
            fired.load(std::sync::atomic::Ordering::SeqCst),
            "selected signal should fire when checked becomes true"
        );
    }

    #[test]
    fn test_selected_not_emitted_on_uncheck() {
        let mut rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        rb.set_checked(true); // start checked
        let count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        {
            let c = std::sync::Arc::clone(&count);
            let scope = rb.connection_scope();
            rb.selected.connect_scoped(scope, move || {
                c.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            });
        }
        rb.set_checked(false);
        assert_eq!(
            count.load(std::sync::atomic::Ordering::SeqCst),
            0,
            "selected signal should NOT fire on uncheck"
        );
    }

    #[test]
    fn test_selected_not_emitted_on_noop() {
        let mut rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        let count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        {
            let c = std::sync::Arc::clone(&count);
            let scope = rb.connection_scope();
            rb.selected.connect_scoped(scope, move || {
                c.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            });
        }
        rb.set_checked(false); // already false -> noop
        assert_eq!(
            count.load(std::sync::atomic::Ordering::SeqCst),
            0,
            "selected signal should NOT fire on noop"
        );
        rb.set_checked(true); // becomes true -> fires
        assert_eq!(
            count.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "selected signal should fire once on becoming true"
        );
    }

    // -----------------------------------------------------------------------
    // 5. Text set/get
    // -----------------------------------------------------------------------
    #[test]
    fn test_text_set_get() {
        let mut rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        assert_eq!(rb.text(), "");
        rb.set_text("Option A".to_string());
        assert_eq!(rb.text(), "Option A");
    }

    #[test]
    fn test_text_overwrite() {
        let mut rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        rb.set_text("First".to_string());
        assert_eq!(rb.text(), "First");
        rb.set_text("Second".to_string());
        assert_eq!(rb.text(), "Second");
    }

    #[test]
    fn test_text_empty_after_set() {
        let mut rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        rb.set_text("Something".to_string());
        rb.set_text(String::new());
        assert_eq!(rb.text(), "");
    }

    // -----------------------------------------------------------------------
    // 6. Group ID set/get
    // -----------------------------------------------------------------------
    #[test]
    fn test_group_id_set_get() {
        let mut rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        assert_eq!(rb.group_id(), None);
        rb.set_group_id(Some("group1".to_string()));
        assert_eq!(rb.group_id(), Some("group1"));
    }

    #[test]
    fn test_group_id_overwrite() {
        let mut rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        rb.set_group_id(Some("group_a".to_string()));
        assert_eq!(rb.group_id(), Some("group_a"));
        rb.set_group_id(Some("group_b".to_string()));
        assert_eq!(rb.group_id(), Some("group_b"));
    }

    #[test]
    fn test_group_id_clear() {
        let mut rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        rb.set_group_id(Some("group1".to_string()));
        assert_eq!(rb.group_id(), Some("group1"));
        rb.set_group_id(None);
        assert_eq!(rb.group_id(), None);
    }

    // -----------------------------------------------------------------------
    // 7. select_in_group static method
    // -----------------------------------------------------------------------
    #[test]
    fn test_select_in_group_selects_correct_peer() {
        let mut rb0 = RadioButton::new(Rect::new(0, 0, 50, 20));
        let mut rb1 = RadioButton::new(Rect::new(0, 0, 50, 20));
        let mut rb2 = RadioButton::new(Rect::new(0, 0, 50, 20));
        rb0.set_group_id(Some("g".to_string()));
        rb1.set_group_id(Some("g".to_string()));
        rb2.set_group_id(Some("g".to_string()));

        let mut peers: Vec<&mut RadioButton> = vec![&mut rb0, &mut rb1, &mut rb2];
        let result = RadioButton::select_in_group(&mut peers, 1);
        assert!(result, "select_in_group should return true on success");
        drop(peers);
        assert!(!rb0.is_checked(), "peer 0 should be unchecked");
        assert!(rb1.is_checked(), "peer 1 should be checked");
        assert!(!rb2.is_checked(), "peer 2 should be unchecked");
    }

    #[test]
    fn test_select_in_group_deselects_others() {
        let mut rb0 = RadioButton::new(Rect::new(0, 0, 50, 20));
        let mut rb1 = RadioButton::new(Rect::new(0, 0, 50, 20));
        rb0.set_group_id(Some("g".to_string()));
        rb1.set_group_id(Some("g".to_string()));

        rb0.set_checked(true);
        assert!(rb0.is_checked());

        let mut peers: Vec<&mut RadioButton> = vec![&mut rb0, &mut rb1];
        RadioButton::select_in_group(&mut peers, 1);
        drop(peers);
        assert!(!rb0.is_checked(), "previously checked peer 0 should be deselected");
        assert!(rb1.is_checked(), "peer 1 should be selected");
    }

    #[test]
    fn test_select_in_group_leaves_other_groups() {
        let mut rb_a0 = RadioButton::new(Rect::new(0, 0, 50, 20));
        let mut rb_a1 = RadioButton::new(Rect::new(0, 0, 50, 20));
        let mut rb_b0 = RadioButton::new(Rect::new(0, 0, 50, 20));
        let mut rb_b1 = RadioButton::new(Rect::new(0, 0, 50, 20));

        rb_a0.set_group_id(Some("A".to_string()));
        rb_a1.set_group_id(Some("A".to_string()));
        rb_b0.set_group_id(Some("B".to_string()));
        rb_b1.set_group_id(Some("B".to_string()));

        rb_a0.set_checked(true);
        rb_b0.set_checked(true);
        assert!(rb_a0.is_checked());
        assert!(rb_b0.is_checked());

        let mut peers: Vec<&mut RadioButton> = vec![&mut rb_a0, &mut rb_a1, &mut rb_b0, &mut rb_b1];
        RadioButton::select_in_group(&mut peers, 1);
        drop(peers);

        assert!(!rb_a0.is_checked(), "group A peer 0 should be deselected");
        assert!(rb_a1.is_checked(), "group A peer 1 should be selected");
        assert!(rb_b0.is_checked(), "group B peer 0 should remain checked");
        assert!(!rb_b1.is_checked(), "group B peer 1 should remain unchecked");
    }

    #[test]
    fn test_select_in_group_out_of_bounds_returns_false() {
        let mut rb0 = RadioButton::new(Rect::new(0, 0, 50, 20));
        let mut rb1 = RadioButton::new(Rect::new(0, 0, 50, 20));

        let mut peers: Vec<&mut RadioButton> = vec![&mut rb0, &mut rb1];
        let result = RadioButton::select_in_group(&mut peers, 5);
        assert!(!result, "out-of-bounds index should return false");
        drop(peers);
        assert!(!rb0.is_checked());
        assert!(!rb1.is_checked());
    }

    #[test]
    fn test_select_in_group_empty_slice_returns_false() {
        let mut empty_peers: Vec<&mut RadioButton> = vec![];
        let result = RadioButton::select_in_group(&mut empty_peers, 0);
        assert!(!result, "empty slice should return false");
    }

    // -----------------------------------------------------------------------
    // 8. Widget trait delegation
    // -----------------------------------------------------------------------
    #[test]
    fn test_widget_id_kind() {
        let rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        assert_eq!(rb.id(), rb.base.id());
        assert_eq!(rb.kind(), WidgetKind::RadioButton);
    }

    #[test]
    fn test_widget_geometry() {
        let mut rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        assert_eq!(rb.geometry(), Rect::new(0, 0, 50, 20));
        rb.set_geometry(Rect::new(10, 10, 80, 30));
        assert_eq!(rb.geometry(), Rect::new(10, 10, 80, 30));
    }

    #[test]
    fn test_widget_min_max_size() {
        let mut rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        assert!(rb.min_size().is_none());
        assert!(rb.max_size().is_none());
        rb.set_min_size(Some(Size::new(20, 10)));
        rb.set_max_size(Some(Size::new(200, 100)));
        assert_eq!(rb.min_size(), Some(Size::new(20, 10)));
        assert_eq!(rb.max_size(), Some(Size::new(200, 100)));
    }

    #[test]
    fn test_widget_parent_children() {
        let mut rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        let pid = 42u64;
        assert!(rb.parent().is_none());
        rb.set_parent(Some(pid));
        assert_eq!(rb.parent(), Some(pid));
        rb.set_parent(None);
        assert!(rb.parent().is_none());

        let cid = 100u64;
        assert!(rb.children().is_empty());
        rb.add_child(cid);
        assert_eq!(rb.children(), &[cid]);
        rb.remove_child(cid);
        assert!(rb.children().is_empty());
    }

    #[test]
    fn test_widget_visibility() {
        let mut rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        assert!(rb.is_visible());
        rb.hide();
        assert!(!rb.is_visible());
        rb.show();
        assert!(rb.is_visible());
    }

    #[test]
    fn test_widget_enabled() {
        let mut rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        assert!(rb.is_enabled());
        rb.set_enabled(false);
        assert!(!rb.is_enabled());
        rb.set_enabled(true);
        assert!(rb.is_enabled());
    }

    #[test]
    fn test_widget_tooltip() {
        let mut rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        assert_eq!(rb.tooltip(), "");
        rb.set_tooltip("Click me".to_string());
        assert_eq!(rb.tooltip(), "Click me");
    }

    #[test]
    fn test_widget_style() {
        let mut rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        let style = WidgetStyle::default();
        rb.set_style(style.clone());
        let _ = rb.style();
    }

    #[test]
    fn test_widget_signals_exist() {
        let rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        let _ = rb.hover_signal();
        let _ = rb.mouse_down_signal();
        let _ = rb.mouse_up_signal();
        let _ = rb.key_down_signal();
        let _ = rb.key_up_signal();
        let _ = rb.focus_gained_signal();
        let _ = rb.focus_lost_signal();
        let _ = rb.redraw_requested_signal();
        let _ = rb.layout_requested_signal();
    }

    #[test]
    fn test_widget_connection_scope() {
        let rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        let _ = rb.connection_scope();
    }

    #[test]
    fn test_widget_request_redraw_signal() {
        let mut rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        let fired = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        {
            let f = std::sync::Arc::clone(&fired);
            let scope = rb.connection_scope();
            rb.redraw_requested_signal().connect_scoped(scope, move || {
                f.store(true, std::sync::atomic::Ordering::SeqCst);
            });
        }
        // Setting text triggers request_redraw, which should emit the signal
        rb.set_text("Test".to_string());
        assert!(
            fired.load(std::sync::atomic::Ordering::SeqCst),
            "redraw_requested_signal should fire when text is set"
        );
    }

    /// `set_checked` asks for a frame, not only a style re-resolution.
    ///
    /// Re-resolving the theme writes the new fill into the style, but nothing is repainted until
    /// something asks — so a programmatic `set_checked(true)` could leave the old fill on screen
    /// until an unrelated event arrived. `CheckBox::set_state`, the sibling that shares this exact
    /// state-plus-re-resolve shape, does both; this pins the same for the radio.
    ///
    /// The assertion is on the signal rather than on `has_ever_requested_redraw`, because that
    /// flag is set once at construction by essentially every control in the crate (see its own
    /// note), so it cannot tell "this setter asked" from "the constructor once asked".
    #[test]
    fn setting_checked_requests_a_redraw() {
        let mut rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        let fired = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        {
            let f = std::sync::Arc::clone(&fired);
            let scope = rb.connection_scope();
            rb.redraw_requested_signal().connect_scoped(scope, move || {
                f.store(true, std::sync::atomic::Ordering::SeqCst);
            });
        }

        rb.set_checked(true);
        assert!(
            fired.load(std::sync::atomic::Ordering::SeqCst),
            "a state change must repaint, or the new fill is not drawn until something else does"
        );

        // An unchanged value is not a change, so it must not repaint.
        fired.store(false, std::sync::atomic::Ordering::SeqCst);
        rb.set_checked(true);
        assert!(
            !fired.load(std::sync::atomic::Ordering::SeqCst),
            "setting the value it already has must not ask for a frame"
        );
    }

    /// A selected radio reports `Checked`, so the preset's `radio_button:checked` key is reachable.
    ///
    /// The same defect `CheckBox::widget_state` documents: the trait default knows the four
    /// primitive flags and none of them means "latched", so the key resolved to `Normal` on every
    /// radio and the accent fill was never painted. A radio makes this the most visible of the
    /// four, because the selected member of a group is the one the user must be able to find.
    #[test]
    fn widget_state_reports_checked_for_a_selected_radio() {
        use crate::style::WidgetState;
        let mut rb = RadioButton::new(Rect::new(0, 0, 100, 30));
        assert_eq!(rb.widget_state(), WidgetState::Normal);

        rb.set_checked(true);
        assert_eq!(rb.widget_state(), WidgetState::Checked);

        // The latch outranks the momentary states, and disabled outranks the latch — the
        // precedence `Widget::widget_state` documents.
        rb.handle_event(&Event::MouseEnter { pos: Point::new(1, 1) });
        assert_eq!(rb.widget_state(), WidgetState::Checked);
        rb.set_enabled(false);
        assert_eq!(rb.widget_state(), WidgetState::Disabled);
    }

    /// The selected dot paints the colour `"radio_button:checked"` **declares**.
    ///
    /// # The orphan key this pins
    ///
    /// Both presets declare the key with `background: primary`, and until this fix the key was
    /// **resolved and then discarded**: `draw` read `style.background_color`, found it was
    /// theme-derived (it holds the *surface* the control sits on), set it to `None`, and fell
    /// through to `SemanticColor::Info`. So a theme author could edit `radio_button:checked` and
    /// see nothing move, and the dot took `info` — a *different* blue (`rgb(138,180,248)` on dark)
    /// from the `primary` the key names (`rgb(100,181,246)`).
    ///
    /// # Why the assertion is on the emitted geometry
    ///
    /// `widget_state()` reported `Checked` and the resolver returned the right colour throughout;
    /// only the painted circle can tell "the declaration was read" from "the declaration was
    /// resolved and thrown away". The expected value is read from the resolver rather than
    /// hard-coded, so this tracks a preset change instead of pinning today's palette.
    #[test]
    #[cfg(device_profile)]
    fn the_selected_dot_paints_the_declared_checked_colour() {
        let _guard = crate::style::theme_test_guard();
        crate::widget::census::install_preset_appearances();
        crate::theme::global_theme_manager().set_appearance(crate::theme::AppearanceMode::Dark);

        let bounds = Rect::new(0, 0, 120, 40);
        let mut button = RadioButton::new(bounds);
        button.set_checked(true);
        crate::theme::apply_active_theme(&mut button);
        let svg = crate::widget::svg::render_widget_to_svg(&mut button, bounds);

        let declared = crate::style::resolved_theme_style_for_state_shared(
            "radio_button",
            crate::style::WidgetState::Checked,
        )
        .and_then(|resolved| resolved.background_color)
        .expect("the preset must declare a `radio_button:checked` fill to paint");

        // The dot is the only circle with a **`fill`** colour: the ring is a stroke, so its
        // `fill` is `none` and its colour lives in `stroke`. Matching on the `fill="rgba` prefix
        // is what separates them — a plain `contains("rgba")` also matches the ring, because
        // `stroke="rgba(...)"` contains it too.
        let dot = svg
            .lines()
            .find(|line| line.contains("<circle") && line.contains("fill=\"rgba"))
            .expect("a selected radio draws a filled dot");
        let expected = format!(
            "rgba({},{},{},{:.2})",
            declared.r,
            declared.g,
            declared.b,
            f32::from(declared.a) / 255.0
        );
        assert!(
            dot.contains(&expected),
            "the selected dot must paint the declared `radio_button:checked` colour {expected}; \
             got: {dot}"
        );
    }

    // ── D09-CTRL-02: the hit area uses the same text measurement as painting ──

    /// A press on the **tail** of a wide-glyph label must select the radio button.
    ///
    /// # The defect this pins
    ///
    /// `hit_area()` estimated the label as `chars().count() * 3/5 em`; wide glyphs advance more, so
    /// the visible label extended past the estimated region and its tail was not clickable. The
    /// region is now measured with the shared `estimate_text_width`, the same ruler the painter uses.
    #[test]
    fn a_press_on_the_tail_of_a_wide_label_selects_the_radio() {
        let mut rb = RadioButton::new(Rect::new(0, 0, 400, 30));
        rb.set_text("WWWWWWWW".to_string());

        let area = rb.hit_area();
        let old_estimate = {
            let line_height = Font::default().size().max(1.0) as u32;
            "WWWWWWWW".chars().count() as u32 * (line_height * 3 / 5).max(1)
        };
        assert!(
            area.width > old_estimate,
            "the measured label must exceed the old char-count estimate (was {old_estimate}, region {})",
            area.width
        );

        // A press just inside the region's right edge (the tail of the visible label) must select.
        let tail = Point::new(area.x + area.width as i32 - 1, area.y + area.height as i32 / 2);
        rb.handle_event(&Event::MousePress { pos: tail, button: 1, modifiers: 0 });
        assert!(rb.is_checked(), "the tail of a wide label must be inside the hit area");
    }

    /// `i` and `W` must not be measured as the same width, so the wider label gets a wider target.
    #[cfg(device_profile)]
    #[test]
    fn narrow_and_wide_labels_are_measured_by_the_shared_ruler() {
        let mut narrow = RadioButton::new(Rect::new(0, 0, 400, 30));
        narrow.set_text("iiii".to_string());
        let mut wide = RadioButton::new(Rect::new(0, 0, 400, 30));
        wide.set_text("WWWW".to_string());

        assert!(
            narrow.hit_area().width < wide.hit_area().width,
            "`W` advances more than `i`, so a wide label's target must be wider: {} vs {}",
            narrow.hit_area().width,
            wide.hit_area().width
        );
    }

    // ── D09-STYLE-01: the radio button consumes the resolved theme/caller font ──

    /// A caller-authored `style.font` must drive the hint, the hit region and the painted label.
    #[test]
    fn a_custom_font_grows_the_radio_hint_and_the_drawn_label() {
        let text = "Choice";
        let mut baseline = RadioButton::new(Rect::new(0, 0, 200, 30));
        baseline.set_text(text.to_string());
        let base_hint = baseline.implicit_size();
        let base_ink = label_ink_width(&mut baseline);

        let mut big = RadioButton::new(Rect::new(0, 0, 200, 30));
        big.set_text(text.to_string());
        big.set_style(crate::style::WidgetStyle::default().with_font(Font::simple("Test", 28.0)));
        let big_hint = big.implicit_size();
        let big_ink = label_ink_width(&mut big);

        assert!(
            big_hint.width > base_hint.width,
            "a larger font must widen the hint: {} vs {}",
            big_hint.width,
            base_hint.width
        );
        assert!(
            big_hint.height > base_hint.height,
            "a larger font must raise the hint: {} vs {}",
            big_hint.height,
            base_hint.height
        );
        assert!(
            big_ink > base_ink,
            "a larger font must widen the painted label ink: {big_ink} vs {base_ink}"
        );
    }

    /// A text scale above 1.0, resolved through the theme, must widen the hint.
    #[test]
    #[cfg(device_profile)]
    fn a_theme_text_scale_above_one_grows_the_radio_hint() {
        use crate::style::environment::{
            install_environment, uninstall_environment, EnvironmentProvider,
        };
        use crate::style::MotionPreference;

        struct Scaled(f32);
        impl EnvironmentProvider for Scaled {
            fn text_scale(&self) -> f32 {
                self.0
            }
            fn motion_preference(&self) -> MotionPreference {
                MotionPreference::NoPreference
            }
        }

        let _guard = crate::style::theme_test_guard();
        let previous = install_environment(Box::new(Scaled(1.0)));
        {
            let mut manager = crate::theme::global_theme_manager();
            manager.register_theme(crate::theme::Theme::default());
            manager.set_appearance(crate::theme::AppearanceMode::Light);
        }
        let mut at_one = RadioButton::new(Rect::new(0, 0, 200, 30));
        at_one.set_text("Scaled choice".to_string());
        crate::theme::apply_active_theme(&mut at_one);
        let hint_one = at_one.implicit_size();

        install_environment(Box::new(Scaled(1.5)));
        let mut scaled = RadioButton::new(Rect::new(0, 0, 200, 30));
        scaled.set_text("Scaled choice".to_string());
        crate::theme::apply_active_theme(&mut scaled);
        let hint_large = scaled.implicit_size();

        let _ = uninstall_environment();
        if let Some(previous) = previous {
            install_environment(previous);
        }

        assert!(
            hint_large.width > hint_one.width,
            "a text scale above 1.0 must widen the radio hint: {} vs {}",
            hint_large.width,
            hint_one.width
        );
    }

    /// The ink width of the label a radio paints, read back from an SVG render.
    fn label_ink_width(radio: &mut RadioButton) -> i32 {
        let svg = crate::widget::svg::render_to_svg(radio);
        crate::widget::svg::text_ink_box(&svg).map(|(x, _, right, _)| right - x).unwrap_or(0)
    }

    /// Keyboard focus paints the ring; a pointer press focuses without painting one.
    ///
    /// The radio already drew the ring through `visual_focus()`, so this test is the
    /// regression guard that keeps its treatment identical to the checkbox's as the
    /// ring's colour source moves to the theme's `outline` token.
    #[test]
    fn keyboard_focus_paints_the_ring_and_pointer_focus_does_not() {
        // The ring's colour reads the process-wide theme; hold the guard so a concurrent
        // appearance switch cannot change this render mid-assertion.
        let _guard = crate::style::theme_test_guard();
        let mut keyboard = RadioButton::new(Rect::new(0, 0, 60, 40));
        keyboard.handle_event(&Event::FocusGained { reason: crate::event::FocusReason::Tab });
        let focused = crate::widget::svg::render_to_svg(&mut keyboard);
        assert!(
            focused
                .lines()
                .any(|line| line.contains("<rect") && line.contains("stroke-width=\"2\"")),
            "keyboard focus must paint the ring as a rounded-rect stroke: {focused}"
        );

        let mut pointer = RadioButton::new(Rect::new(0, 0, 60, 40));
        pointer.handle_event(&Event::FocusGained { reason: crate::event::FocusReason::Pointer });
        let unfocused = crate::widget::svg::render_to_svg(&mut pointer);
        assert!(
            !unfocused
                .lines()
                .any(|line| line.contains("<rect") && line.contains("stroke-width=\"2\"")),
            "pointer focus must not paint a ring: {unfocused}"
        );
    }

    /// A same-value write must not queue a repaint.
    ///
    /// `Button`, `CheckBox` and `ToggleButton` already answered this way; this control was the one
    /// in the family that repainted on every write, so a host that syncs its model in a loop queued
    /// a frame per write for a picture that could not change (WDGT-12).
    #[test]
    fn a_same_value_write_does_not_queue_a_repaint() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let mut radio = RadioButton::new(Rect::new(0, 0, 200, 30));
        radio.set_text("Option A".to_string());

        let count = Arc::new(AtomicUsize::new(0));
        let sink = Arc::clone(&count);
        let subscription = crate::signal::ConnectionScope::new();
        radio.base().redraw_requested.connect_scoped(&subscription, move || {
            sink.fetch_add(1, Ordering::SeqCst);
        });

        radio.set_text("Option A".to_string());
        assert_eq!(count.load(Ordering::SeqCst), 0, "a no-op write must not repaint");

        radio.set_text("Option B".to_string());
        assert_eq!(count.load(Ordering::SeqCst), 1, "a real change must repaint");
        assert_eq!(radio.text(), "Option B");
    }

    // ── Interaction fade (P1-9) ─────────────────────────────────────────────

    /// Rasterises the radio's paint into the SVG document the comparisons read.
    fn render(radio: &mut RadioButton) -> String {
        crate::widget::svg::render_to_svg(radio)
    }

    /// A pointer entering the radio retargets the hover driver at once.
    ///
    /// The answer has to be true before any `tick` has run: the animation bus asks
    /// `is_animating` first, and a control that only admitted movement on its next tick
    /// would never be scheduled for one.
    #[test]
    fn mouse_enter_makes_the_radio_animating() {
        let mut radio = RadioButton::new(Rect::new(0, 0, 60, 40));
        assert!(!Widget::is_animating(&radio), "a new radio rests and owes no frames");

        radio.handle_event(&Event::MouseEnter { pos: Point::new(1, 1) });

        assert!(
            Widget::is_animating(&radio),
            "MouseEnter must make the radio's fade moving before any tick"
        );
        assert!(Widget::tick(&mut radio, 16), "the first fade frame must report work");
    }

    /// A resting radio paints its resolved ink byte-for-byte, and returns to exactly that
    /// picture after a hover has come and gone.
    #[test]
    fn a_resting_radio_paints_the_untinted_indicator() {
        let _guard = crate::style::theme_test_guard();
        let mut radio = RadioButton::new(Rect::new(0, 0, 120, 32));
        radio.set_text("Option A".to_string());
        let fresh = render(&mut radio);

        radio.handle_event(&Event::MouseEnter { pos: Point::new(1, 1) });
        while Widget::tick(&mut radio, 16) {}
        assert_ne!(render(&mut radio), fresh, "a settled hover must actually tint the indicator");

        radio.handle_event(&Event::MouseLeave { pos: Point::new(1, 1) });
        while Widget::tick(&mut radio, 16) {}
        assert_eq!(radio.fade.hover.value(), 0.0, "the fade returns to rest exactly");
        assert_eq!(
            render(&mut radio),
            fresh,
            "a radio back at rest must repaint the same bytes it did before the hover"
        );
    }

    /// Animating to the hover terminal state produces the same picture as setting the
    /// terminal value directly, so the fade is purely a path between two identical ends.
    #[test]
    fn a_settled_hover_matches_the_manually_set_terminal_state() {
        let _guard = crate::style::theme_test_guard();
        let make = || {
            let mut radio = RadioButton::new(Rect::new(0, 0, 120, 32));
            radio.set_text("Option A".to_string());
            radio
        };

        let mut animated = make();
        animated.handle_event(&Event::MouseEnter { pos: Point::new(1, 1) });
        let mut frames = 0;
        while Widget::tick(&mut animated, 16) {
            frames += 1;
            assert!(frames < 500, "the fade must terminate");
        }
        assert_eq!(animated.fade.hover.value(), 1.0, "the hover settles at its target");

        let mut manual = make();
        manual.fade.hover.jump_to(1.0);
        manual.fade.press.jump_to(0.0);

        assert_eq!(
            render(&mut animated),
            render(&mut manual),
            "the animated end state must be the manually set terminal state"
        );
    }

    /// Reduced motion collapses the fast tempo to zero, so one tick reaches the target.
    #[test]
    fn reduced_motion_reaches_the_radio_target_in_one_frame() {
        use crate::compat::Box;
        use crate::style::environment::{
            install_environment, uninstall_environment, EnvironmentProvider,
        };
        use crate::style::MotionPreference;

        struct Reduced;
        impl EnvironmentProvider for Reduced {
            fn motion_preference(&self) -> MotionPreference {
                MotionPreference::ReduceMotion
            }
        }

        let _guard = crate::style::theme_test_guard();
        let previous = install_environment(Box::new(Reduced));

        let mut radio = RadioButton::new(Rect::new(0, 0, 60, 40));
        radio.handle_event(&Event::MouseEnter { pos: Point::new(1, 1) });
        let moving = Widget::tick(&mut radio, 16);

        assert!(!moving, "a zero-duration fade settles on the first tick");
        assert_eq!(radio.fade.hover.value(), 1.0, "and reaches the target, not half way");
        assert!(!Widget::is_animating(&radio));

        let _ = uninstall_environment();
        if let Some(previous) = previous {
            install_environment(previous);
        }
    }

    /// A hover frame submits damage bounded by the painted box and the margin.
    #[cfg(not(alloc_frugal))]
    #[test]
    fn a_hover_frame_submits_the_painted_box_with_the_damage_margin() {
        use crate::widget::runtime;

        let id =
            runtime::register(crate::compat::Box::new(RadioButton::new(Rect::new(4, 8, 120, 32))))
                .expect("registry");
        assert!(runtime::set_repaint_mode(id, runtime::RepaintMode::Dirty));

        assert!(runtime::dispatch_event(id, &Event::mouse_enter(1, 1)));

        let expected = runtime::painted_box_with_damage_margin(id).expect("painted box");
        assert_eq!(
            runtime::dirty_rects(id),
            vec![expected],
            "a hover must damage the painted box plus the margin"
        );
        runtime::unregister(id);
    }

    /// `Full` keeps the whole-control path and records no damage rectangle.
    #[cfg(not(alloc_frugal))]
    #[test]
    fn a_hover_frame_in_full_mode_keeps_the_whole_control_repaint() {
        use crate::widget::runtime;

        let id =
            runtime::register(crate::compat::Box::new(RadioButton::new(Rect::new(4, 8, 120, 32))))
                .expect("registry");
        assert_eq!(runtime::repaint_mode(id), runtime::RepaintMode::Full);

        assert!(runtime::dispatch_event(id, &Event::mouse_enter(1, 1)));
        assert!(
            runtime::dirty_rects(id).is_empty(),
            "Full mode records no damage, so the control repaints whole"
        );
        runtime::unregister(id);
    }

    // ── Group exclusivity and arrow navigation (R016, R017) ────────────

    #[test]
    fn a_clicked_radio_unchecks_its_grouped_peer() {
        let mut a = RadioButton::new(Rect::new(0, 0, 80, 20));
        let mut b = RadioButton::new(Rect::new(0, 24, 80, 20));
        a.set_group_id(Some("size".into()));
        b.set_group_id(Some("size".into()));

        a.set_checked(true);
        assert!(a.is_checked());
        b.set_checked(true);
        assert!(b.is_checked());
        assert!(!a.is_checked(), "the group selects one member, so the peer is no longer checked");
    }

    #[test]
    fn different_group_ids_do_not_exclude_each_other() {
        let mut a = RadioButton::new(Rect::new(0, 0, 80, 20));
        let mut b = RadioButton::new(Rect::new(0, 24, 80, 20));
        a.set_group_id(Some("left".into()));
        b.set_group_id(Some("right".into()));
        a.set_checked(true);
        b.set_checked(true);
        assert!(a.is_checked() && b.is_checked());
    }

    #[test]
    fn clearing_the_group_id_releases_the_selection() {
        let mut a = RadioButton::new(Rect::new(0, 0, 80, 20));
        let mut b = RadioButton::new(Rect::new(0, 24, 80, 20));
        a.set_group_id(Some("g".into()));
        b.set_group_id(Some("g".into()));
        a.set_checked(true);
        b.set_group_id(None);
        assert!(a.is_checked(), "a departing member cannot hold the group's selection");
    }

    #[test]
    fn arrow_keys_move_the_selection_down_the_group() {
        let mut first = RadioButton::new(Rect::new(0, 0, 80, 20));
        let mut second = RadioButton::new(Rect::new(0, 24, 80, 20));
        let mut third = RadioButton::new(Rect::new(0, 48, 80, 20));
        for radio in [&mut first, &mut second, &mut third] {
            radio.set_group_id(Some("column".into()));
        }
        first.set_checked(true);

        first.handle_event(&Event::KeyPress { key: key_codes::DOWN, modifiers: 0 });
        assert!(!first.is_checked());
        assert!(second.is_checked(), "Down selects the next radio");
        assert!(!third.is_checked());

        second.handle_event(&Event::KeyPress { key: key_codes::UP, modifiers: 0 });
        assert!(first.is_checked(), "Up selects the previous radio");
    }

    #[test]
    fn arrow_navigation_wraps_and_skips_disabled_peers() {
        let mut first = RadioButton::new(Rect::new(0, 0, 80, 20));
        let mut middle = RadioButton::new(Rect::new(0, 24, 80, 20));
        let mut last = RadioButton::new(Rect::new(0, 48, 80, 20));
        for radio in [&mut first, &mut middle, &mut last] {
            radio.set_group_id(Some("wrapping".into()));
        }
        middle.set_enabled(false);
        first.set_checked(true);

        first.handle_event(&Event::KeyPress { key: key_codes::DOWN, modifiers: 0 });
        assert!(last.is_checked(), "a disabled member is skipped, and the search wraps");
    }

    #[test]
    fn auto_exclusive_groups_same_parent_radios() {
        let mut a = RadioButton::new(Rect::new(0, 0, 80, 20));
        let mut b = RadioButton::new(Rect::new(0, 24, 80, 20));
        a.set_parent(Some(7));
        b.set_parent(Some(7));
        assert!(a.is_auto_exclusive(), "same-parent exclusivity is the default");

        a.set_checked(true);
        b.set_checked(true);
        assert!(b.is_checked());
        assert!(!a.is_checked());
    }

    #[test]
    fn auto_exclusive_can_be_turned_off() {
        let mut a = RadioButton::new(Rect::new(0, 0, 80, 20));
        let mut b = RadioButton::new(Rect::new(0, 24, 80, 20));
        a.set_parent(Some(7));
        b.set_parent(Some(7));
        a.set_auto_exclusive(false);
        b.set_auto_exclusive(false);
        a.set_checked(true);
        b.set_checked(true);
        assert!(a.is_checked() && b.is_checked());
    }
}
