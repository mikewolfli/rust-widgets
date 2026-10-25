// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Toggle button widget.
#[cfg(full_widgets)]
use crate::compat::Vec;
use crate::compat::{Rc, RefCell, String, ToString};
use crate::core::{Font, HorizontalAlignment, ObjectId, Point, Rect, Size};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::style::EdgeOffsets;
use crate::widget::base_widgets::button_group::{self, GroupKey};
use crate::widget::base_widgets::AutoRepeat;
use crate::widget::capability::coercion::{expect_bool, expect_string, expect_u32};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
#[cfg(full_widgets)]
use crate::widget::menu_toolbar::menu_button::{
    draw_menu_panel, menu_item_rect, menu_panel_rect, MenuArrowType, MenuButtonPopup, MenuItem,
};
use crate::widget::metrics::{dimensions, effective_font, estimate_text_width, ControlMetrics};
use crate::widget::{BaseWidget, Draw, IconSource, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
/// Toggle button state enumeration.
/// # Why the two interaction states are derived rather than stored
///
/// This enum used to have exactly the two *persistent* states (`Checked` / `Normal`) plus
/// `Disabled`, so `draw` had no way to express "the pointer is over this" or "this is being held"
/// — the control's own `is_pressed` flag was maintained and then never read. A user toggling a
/// button therefore got no feedback at all until the latch flipped, which is the one moment the
/// feedback was least needed.
///
/// `Hover` and `Pressed` are read off [`BaseWidget`], which is where the crate keeps its single
/// copy of those facts (that is what makes M1 a zero-code win for every control). They sit
/// **below** `Checked`/`Disabled` in the precedence because those are persistent facts about the
/// control while these are momentary: a checked button that is hovered is still usefully called
/// `Checked`, and the hover is then an *overlay* the draw path adds — the same layering
/// [`Widget::widget_state`] describes. Putting it in this order is what keeps the existing
/// `state` property value stable for every caller that already reads it.
///
/// Derived from the checked and enabled flags, never stored: see
/// [`ToggleButton::state`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToggleButtonState {
    /// Enabled, not checked, and not being pointed at or held.
    Normal,
    /// Enabled, not checked, and the pointer is over it.
    Hover,
    /// Enabled, not checked, and currently held down.
    Pressed,
    /// Enabled and checked. Disabled always wins over checked, so an unchecked
    /// *and* disabled button also reports [`ToggleButtonState::Disabled`].
    Checked,
    /// Not enabled; reported regardless of the checked flag.
    Disabled,
}
/// The inputs [`ToggleButton`]'s size hint derives its answer from.
///
/// The same keyed cache `Button` uses, for the same reason: `draw` asks for the hint on every
/// frame, and a hover or interaction fade repaints many frames with unchanged inputs. Comparing
/// the inputs cannot go stale the way a dirty flag could — the font comes from the active theme
/// and the advances come from the icon and arrow properties.
#[derive(Debug, Clone, PartialEq)]
struct ToggleHintKey {
    /// The caption's revision; see `ToggleButton::text_revision`.
    text_revision: u32,
    /// The font the label is measured with.
    font: Font,
    /// The leading icon's advance, zero when there is none.
    icon_advance: u32,
    /// The disclosure arrow's advance, zero when there is none.
    arrow_advance: u32,
}

/// Toggle button: a two-state push button that latches on click.
///
/// Also carries press tracking (`is_pressed`) so it can render a pressed
/// appearance between mouse-down and mouse-up.
pub struct ToggleButton {
    base: BaseWidget,
    text: String,
    checked: bool,
    auto_exclusive: bool,
    group_id: Option<Rc<str>>,
    /// Whether the tool button hides its chrome until it is pointed at, held or checked.
    auto_raise: bool,
    /// A typed icon painted before the caption.
    icon_source: Option<IconSource>,
    /// Which direction the menu disclosure arrow points.
    #[cfg(full_widgets)]
    arrow_type: MenuArrowType,
    /// The attached menu's rows.
    #[cfg(full_widgets)]
    menu_items: Vec<MenuItem>,
    /// Whether the attached menu is showing.
    #[cfg(full_widgets)]
    menu_open: bool,
    /// How the face and the disclosure arrow divide a press.
    #[cfg(full_widgets)]
    popup_mode: MenuButtonPopup,
    /// The highlighted row while the menu is open.
    #[cfg(full_widgets)]
    active_menu_index: Option<usize>,
    /// How long a [`MenuButtonPopup::DelayedPopup`] press must be held before the menu opens.
    #[cfg(full_widgets)]
    popup_delay_ms: u32,
    /// Elapsed milliseconds of the current held press, when one is timing.
    #[cfg(full_widgets)]
    press_elapsed: Option<u32>,
    /// The auto-repeat timing while this button is held.
    ///
    /// Off by default, so a tool button that never opts in still toggles once per click. Advanced
    /// from [`ToggleButton::tick`] out of the same clock the interaction transition uses.
    auto_repeat: AutoRepeat,
    /// A monotonic counter bumped whenever the caption changes.
    text_revision: u32,
    /// The cached intrinsic size, with the inputs it was derived from.
    hint_cache: RefCell<Option<(ToggleHintKey, Size)>>,
    /// Emitted when a row of the attached menu is activated.
    #[cfg(full_widgets)]
    pub item_triggered: Signal1<u64>,
    /// The last value `pressed_signal`/`released_signal` were emitted for.
    ///
    /// The *paint* flag is [`BaseWidget::is_pressed`] (the crate's single source); this
    /// is only the edge latch that decides whether the signals have been announced, so a
    /// no-op repeat set cannot re-emit. See `Button::signaled_pressed` for the full
    /// argument.
    signaled_pressed: bool,
    /// Emitted with the new checked flag whenever it changes. Semantically a
    /// synonym for `checked_changed`, kept for callers using the checked-state
    /// terminology.
    pub toggled: Signal1<bool>,
    /// Emitted with the new checked flag whenever it changes.
    pub checked_changed: Signal1<bool>,
    /// Emitted on the rising edge of the pressed flag (mouse down).
    pub pressed_signal: GenericSignal,
    /// Emitted on the falling edge of the pressed flag (mouse up).
    pub released_signal: GenericSignal,
    /// Emitted with the recomputed [`ToggleButtonState`] whenever the checked
    /// flag changes. Not emitted when only the enabled flag changes, so a
    /// disabled button can still report a stale `Normal`. Reading
    /// [`ToggleButton::state`] after `set_enabled` gives the current value.
    pub state_changed: Signal1<ToggleButtonState>,
    /// The interpolated interaction progress, 0.0 at rest and 1.0 fully pressed.
    ///
    /// The same type and the same contract as `Button::interaction_progress`. It exists so a
    /// hover fades in and a press deepens it instead of both snapping — and, at progress `0.0`,
    /// the fill is exactly the resting colour, which is why adding it left every committed
    /// snapshot byte-identical.
    ///
    /// [`crate::style::PropertyDriver`] owns the value **and its target**, so the second
    /// `interaction_target` field this control used to carry is gone — see `Button`'s field
    /// for the same reasoning (BLUE24 §2.3).
    interaction_progress: crate::style::PropertyDriver,
}
impl ToggleButton {
    /// Creates an unchecked, enabled toggle button with the given caption.
    ///
    /// `geometry` is in parent-relative logical pixels.
    pub fn new(text: String, geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ToggleButton, geometry, "ToggleButton"),
            text,
            checked: false,
            auto_exclusive: false,
            group_id: None,
            auto_raise: false,
            icon_source: None,
            #[cfg(full_widgets)]
            arrow_type: MenuArrowType::None,
            #[cfg(full_widgets)]
            menu_items: Vec::new(),
            #[cfg(full_widgets)]
            menu_open: false,
            #[cfg(full_widgets)]
            popup_mode: MenuButtonPopup::InstantPopup,
            #[cfg(full_widgets)]
            active_menu_index: None,
            #[cfg(full_widgets)]
            popup_delay_ms: 300,
            #[cfg(full_widgets)]
            press_elapsed: None,
            auto_repeat: AutoRepeat::new(),
            text_revision: 0,
            hint_cache: RefCell::new(None),
            #[cfg(full_widgets)]
            item_triggered: Signal1::new(),
            signaled_pressed: false,
            toggled: Signal1::new(),
            checked_changed: Signal1::new(),
            pressed_signal: GenericSignal::new(),
            released_signal: GenericSignal::new(),
            state_changed: Signal1::new(),
            interaction_progress: crate::style::PropertyDriver::default(),
        }
    }
    /// Returns the button caption, drawn centered. Empty by default only if
    /// constructed that way (`new` takes the text up front).
    pub fn text(&self) -> &str {
        &self.text
    }
    /// Replaces the caption. No-op (and no redraw) when the text is unchanged.
    pub fn set_text(&mut self, text: impl Into<String>) {
        let text = text.into();
        if self.text != text {
            self.text = text;
            // Invalidates the cached hint through the key rather than by a separate flag.
            self.text_revision = self.text_revision.wrapping_add(1);
            self.base.request_redraw();
        }
    }
    /// Returns the latched checked flag.
    ///
    /// An auto-exclusive button displaced by a selected peer reports `false`: the group holds the
    /// selection and this control reads it back rather than keeping a second copy.
    pub fn is_checked(&self) -> bool {
        match self.exclusive_key() {
            Some(key) => {
                button_group::is_selected(&key, self.id())
                    || (button_group::selected(&key).is_none() && self.checked)
            }
            None => self.checked,
        }
    }

    /// The group this button belongs to, when auto-exclusive is on.
    ///
    /// A group a [`crate::widget::base_widgets::ButtonGroup`] handle adopted this button into wins
    /// over both properties, because it is a direct instruction from the caller.
    fn exclusive_key(&self) -> Option<GroupKey> {
        if let Some(key) = button_group::membership(self.id()) {
            return Some(key);
        }
        if !self.auto_exclusive {
            return None;
        }
        if let Some(id) = self.group_id.as_ref() {
            return Some(GroupKey::Name(Rc::clone(id)));
        }
        self.parent().map(GroupKey::Parent)
    }

    /// Records this button's current facts with its group.
    fn sync_group_registration(&self) {
        if let Some(key) = self.exclusive_key() {
            button_group::register(key, self.id(), self.geometry(), self.base.is_enabled());
        }
    }

    /// Makes this button the group's selected member.
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

    /// Releases the group's selection when this button holds it.
    fn release_group_selection(&mut self) {
        if let Some(key) = self.exclusive_key() {
            if button_group::is_selected(&key, self.id()) {
                button_group::clear_selection(&key);
            }
        }
    }
    /// Sets the checked flag.
    ///
    /// A no-op when the value is unchanged, which means **no signals fire on a
    /// redundant set**. On an actual change this emits `checked_changed` and
    /// `toggled` (both with the new flag) followed by `state_changed`, and
    /// requests a redraw.
    ///
    /// The redraw is not enough on its own: `draw` reads `style.background_color`
    /// before the `checked` fallback below, and the theme wrote that field once — at
    /// creation, in the state the constructor produced. So this asks the theme to
    /// re-resolve `"toggle_button:<state>"` first, or the new latch paints the old
    /// colour. See [`crate::style::reapply_active_theme_state`] for the measurement.
    pub fn set_checked(&mut self, checked: bool) {
        let previous = self.is_checked();
        if self.checked == checked && previous == checked {
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
        // Before the signals: a handler that reads the style back must see the state it
        // is being told about, not the previous one.
        crate::style::reapply_active_theme_state(self);
        self.base.request_redraw();
        // Only an effective change is announced: a displaced member clearing its own latch
        // already reported `false` through `is_checked`.
        if previous != checked {
            self.checked_changed.emit(checked);
            self.toggled.emit(checked);
            self.state_changed.emit(self.state());
            button_group::notify_member_event(self.id(), button_group::MemberEvent::Toggled);
        }
    }
    /// Flips the checked flag through [`ToggleButton::set_checked`], so the
    /// usual signals fire.
    pub fn toggle(&mut self) {
        self.set_checked(!self.is_checked());
    }
    /// Returns the auto-exclusive intent flag. Defaults to `false`.
    pub fn is_auto_exclusive(&self) -> bool {
        self.auto_exclusive
    }
    /// Sets the auto-exclusive intent flag.
    ///
    /// With it on, the button actually excludes its peers: checking it unchecks the other
    /// auto-exclusive buttons in the same `group_id` (or under the same parent when no id is set).
    /// Off by default, so an ungrouped tool button keeps the independent latch it always had.
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
    /// Returns the group this button belongs to, or `None` when ungrouped.
    ///
    /// The id is an opaque caller-chosen string; the widget only stores and
    /// reports it. See [`ToggleButton::set_auto_exclusive`].
    pub fn group_id(&self) -> Option<&str> {
        self.group_id.as_deref()
    }
    /// Sets (or clears, with `None`) the group id. See
    /// [`ToggleButton::group_id`].
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
        self.base.request_redraw();
    }

    /// Whether the button hides its chrome until the pointer or the latch lights it.
    pub fn is_auto_raise(&self) -> bool {
        self.auto_raise
    }

    /// Hides the tool button's face and frame at rest.
    ///
    /// The reference toolkit's `autoRaise`: a toolbar can leave its tool buttons flat until they
    /// are hovered or checked. Off by default, so the existing button keeps its bordered look.
    pub fn set_auto_raise(&mut self, auto_raise: bool) {
        if self.auto_raise == auto_raise {
            return;
        }
        self.auto_raise = auto_raise;
        self.base.request_redraw();
    }

    /// The button's typed icon, if one is set.
    pub fn icon_source(&self) -> Option<IconSource> {
        self.icon_source
    }

    /// Sets (or clears) the button's leading icon.
    pub fn set_icon_source(&mut self, source: Option<IconSource>) {
        if self.icon_source == source {
            return;
        }
        self.icon_source = source;
        let _ = self.hint_cache.replace(None);
        self.base.request_redraw();
    }

    /// Which way the menu disclosure arrow points.
    #[cfg(full_widgets)]
    pub fn arrow_type(&self) -> MenuArrowType {
        self.arrow_type
    }

    /// Sets the disclosure arrow's direction. [`MenuArrowType::None`] hides it.
    #[cfg(full_widgets)]
    pub fn set_arrow_type(&mut self, arrow: MenuArrowType) {
        if self.arrow_type == arrow {
            return;
        }
        self.arrow_type = arrow;
        // The arrow's advance is part of the hint's content width.
        let _ = self.hint_cache.replace(None);
        self.base.request_redraw();
    }

    /// The attached menu's rows.
    #[cfg(full_widgets)]
    pub fn menu_items(&self) -> &[MenuItem] {
        &self.menu_items
    }

    /// Attaches a drop-down menu. An empty list removes it.
    #[cfg(full_widgets)]
    pub fn set_menu(&mut self, items: Vec<MenuItem>) {
        self.menu_items = items;
        if self.menu_items.is_empty() {
            self.close_menu();
        }
        self.base.request_redraw();
    }

    /// How a press is divided between the face and the disclosure arrow.
    #[cfg(full_widgets)]
    pub fn popup_mode(&self) -> MenuButtonPopup {
        self.popup_mode
    }

    /// Sets the press split. See [`MenuButtonPopup`].
    #[cfg(full_widgets)]
    pub fn set_popup_mode(&mut self, mode: MenuButtonPopup) {
        if self.popup_mode == mode {
            return;
        }
        self.popup_mode = mode;
        self.base.request_redraw();
    }

    /// Whether the attached menu is open.
    #[cfg(full_widgets)]
    pub fn is_menu_open(&self) -> bool {
        self.menu_open
    }

    /// The menu's panel rectangle.
    #[cfg(full_widgets)]
    pub fn menu_rect(&self) -> Rect {
        menu_panel_rect(self.geometry(), self.menu_items.len())
    }

    /// Opens the attached menu.
    #[cfg(full_widgets)]
    pub fn open_menu(&mut self) {
        if self.menu_items.is_empty() || self.menu_open {
            return;
        }
        self.menu_open = true;
        self.active_menu_index = self.menu_items.iter().position(|item| item.enabled);
        self.base.request_redraw();
    }

    /// Closes the attached menu.
    #[cfg(full_widgets)]
    pub fn close_menu(&mut self) {
        if !self.menu_open {
            return;
        }
        self.menu_open = false;
        self.active_menu_index = None;
        self.base.request_redraw();
    }

    /// Toggles the attached menu.
    #[cfg(full_widgets)]
    pub fn toggle_menu(&mut self) {
        if self.menu_open {
            self.close_menu();
        } else {
            self.open_menu();
        }
    }

    /// The segment of the button that opens the menu directly.
    #[cfg(full_widgets)]
    pub fn arrow_rect(&self) -> Rect {
        let geom = self.geometry();
        if self.arrow_type == MenuArrowType::None {
            return geom;
        }
        let width = geom.width.min(20);
        Rect::new(geom.x + geom.width as i32 - width as i32, geom.y, width, geom.height)
    }

    /// The enabled menu row at `pos`.
    #[cfg(full_widgets)]
    fn hit_test_menu_item(&self, pos: Point) -> Option<usize> {
        if !self.menu_open {
            return None;
        }
        let panel = self.menu_rect();
        if !panel.contains_point(pos) {
            return None;
        }
        self.menu_items.iter().enumerate().find_map(|(index, item)| {
            let row = menu_item_rect(panel, index);
            (item.enabled && row.contains_point(pos)).then_some(index)
        })
    }

    /// Emits `item_triggered` for the row and closes the menu.
    #[cfg(full_widgets)]
    fn activate_menu_row(&mut self, index: usize) {
        let Some(item) = self.menu_items.get(index) else {
            return;
        };
        if !item.enabled {
            return;
        }
        let id = item.id;
        self.item_triggered.emit(id);
        self.close_menu();
    }

    /// Moves the keyboard highlight through the enabled rows.
    #[cfg(full_widgets)]
    fn move_menu_highlight(&mut self, forward: bool) {
        let enabled: Vec<usize> = self
            .menu_items
            .iter()
            .enumerate()
            .filter(|(_, item)| item.enabled)
            .map(|(i, _)| i)
            .collect();
        if enabled.is_empty() {
            return;
        }
        let position = self
            .active_menu_index
            .and_then(|active| enabled.iter().position(|index| *index == active));
        let next = match position {
            Some(position) if forward => (position + 1) % enabled.len(),
            Some(position) => (position + enabled.len() - 1) % enabled.len(),
            None => 0,
        };
        self.active_menu_index = Some(enabled[next]);
        self.base.request_redraw();
    }
    /// Returns whether the button is currently held down.
    pub fn is_pressed(&self) -> bool {
        self.base.is_pressed()
    }
    /// Sets the pressed flag.
    ///
    /// No-op when unchanged, so the signals fire only on an actual edge: a
    /// rising edge emits `pressed_signal`, a falling edge emits
    /// `released_signal`. Unlike the checked flag, this does not request a
    /// redraw.
    pub fn set_pressed(&mut self, pressed: bool) {
        // The trait default carries both the field write and the theme re-resolution for
        // `toggle_button:pressed`; this inherent method shadows it, so it delegates through the
        // trait object rather than writing the field itself.
        <dyn Widget>::set_pressed(self, pressed);
        if self.signaled_pressed == pressed {
            return;
        }
        self.signaled_pressed = pressed;
        // A new gesture starts its repeat delay from zero, even if no tick observed the gap
        // between the old and the new press.
        self.auto_repeat.reset();
        if pressed {
            self.pressed_signal.emit();
            button_group::notify_member_event(self.id(), button_group::MemberEvent::Pressed);
        } else {
            self.released_signal.emit();
            button_group::notify_member_event(self.id(), button_group::MemberEvent::Released);
        }
    }
    /// Returns the interaction state derived from the enabled, checked, hovered
    /// and pressed flags. Disabled takes precedence over checked, and checked over
    /// the two momentary states; see [`ToggleButtonState`] for why that order.
    pub fn state(&self) -> ToggleButtonState {
        if !self.base.enabled {
            ToggleButtonState::Disabled
        } else if self.is_checked() {
            ToggleButtonState::Checked
        } else if self.base.is_pressed() {
            ToggleButtonState::Pressed
        } else if self.base.is_hovered() {
            ToggleButtonState::Hover
        } else {
            ToggleButtonState::Normal
        }
    }

    /// The horizontal room the leading icon takes, including its gap.
    fn icon_advance(&self) -> u32 {
        if self.icon_source.is_some() {
            dimensions::TOOL_BUTTON_ICON_SIZE + dimensions::TOOL_BUTTON_ICON_SPACING
        } else {
            0
        }
    }

    /// The horizontal room the disclosure arrow takes.
    fn arrow_advance(&self) -> u32 {
        #[cfg(full_widgets)]
        {
            if self.arrow_type == MenuArrowType::None {
                0
            } else {
                12
            }
        }
        #[cfg(not(full_widgets))]
        {
            0
        }
    }

    /// Measures the intrinsic size from scratch; the slow path behind `size_hint`.
    fn measure_hint(&self, font: &Font, icon_advance: u32, arrow_advance: u32) -> Size {
        let label_width = estimate_text_width(self.text(), font, 1.0);
        ControlMetrics::implicit_size(
            Size::new(label_width + icon_advance + arrow_advance, dimensions::FONT_SIZE_BASE + 4),
            EdgeOffsets::symmetric(dimensions::BUTTON_PADDING_V, dimensions::BUTTON_PADDING_H),
            dimensions::BUTTON_MIN,
        )
    }

    /// How long a delayed popup press must be held.
    #[cfg(full_widgets)]
    pub fn popup_delay_ms(&self) -> u32 {
        self.popup_delay_ms
    }

    /// Sets the delayed-popup hold time.
    #[cfg(full_widgets)]
    pub fn set_popup_delay_ms(&mut self, delay_ms: u32) {
        self.popup_delay_ms = delay_ms;
    }

    /// Whether holding this button repeats its press/release cycle.
    pub fn is_auto_repeat(&self) -> bool {
        self.auto_repeat.is_enabled()
    }

    /// Enables automatic repetition while the button is held.
    ///
    /// Off by default. When on, holding the button re-emits its `pressed`/`released` pair at
    /// [`Self::auto_repeat_interval`] once the press has been held for
    /// [`Self::auto_repeat_delay`]. The latch does not move on a repeat, so the button keeps its
    /// held appearance while its listeners see each cycle.
    pub fn set_auto_repeat(&mut self, enabled: bool) {
        self.auto_repeat.set_enabled(enabled);
    }

    /// How long the button must be held before the first repeat, in milliseconds.
    pub fn auto_repeat_delay(&self) -> u32 {
        self.auto_repeat.delay_ms()
    }

    /// Sets the delay before the first repeat.
    pub fn set_auto_repeat_delay(&mut self, delay_ms: u32) {
        self.auto_repeat.set_delay_ms(delay_ms);
    }

    /// The gap between repeats while the button is held, in milliseconds.
    pub fn auto_repeat_interval(&self) -> u32 {
        self.auto_repeat.interval_ms()
    }

    /// Sets the gap between repeats.
    pub fn set_auto_repeat_interval(&mut self, interval_ms: u32) {
        self.auto_repeat.set_interval_ms(interval_ms);
    }

    /// Whether this button is held and configured to repeat.
    ///
    /// `set_pressed` is only ever armed by a press that landed on the control, so the base flag
    /// is already the right "held" answer here.
    fn auto_repeat_held(&self) -> bool {
        self.auto_repeat.is_enabled() && self.base.is_enabled() && self.base.is_pressed()
    }

    /// Reports one auto-repeat cycle.
    ///
    /// A `released`/`pressed` pair, exactly as `Button::emit_auto_repeat` does: the latch never
    /// moves, so the button stays painted pressed while its listeners see each repeat.
    fn emit_auto_repeat(&mut self) {
        self.released_signal.emit();
        self.pressed_signal.emit();
        button_group::notify_member_event(self.id(), button_group::MemberEvent::Released);
        button_group::notify_member_event(self.id(), button_group::MemberEvent::Pressed);
    }

    /// Emits `clicked` and reports it to this button's group, if it has one.
    fn emit_clicked(&mut self) {
        self.base.clicked.emit();
        button_group::notify_member_event(self.id(), button_group::MemberEvent::Clicked);
    }

    /// The progress the current interaction state calls for.
    ///
    /// Mirrors `Button::interaction_target_progress`: a press is the furthest point, a hover a
    /// partial step, and a disabled control always rests. The checked state is deliberately **not**
    /// part of this — a latch is a value, not a gesture, and animating it would make the toggle's
    /// fill depend on when the user last clicked rather than on whether it is on.
    fn interaction_target_progress(&self) -> f32 {
        if !self.base.enabled {
            0.0
        } else if self.base.is_pressed() {
            1.0
        } else if self.base.is_hovered() {
            0.5
        } else {
            0.0
        }
    }

    /// Advances the interaction transition by `delta_ms`, reporting whether another
    /// frame is needed.
    ///
    /// The same contract as `Button::tick` — and, like it, the duration is the active theme's
    /// `Motion::normal` rather than a constant in this file, so a theme can state its own tempo
    /// and a test can shorten it to reach the end state deterministically.
    pub fn tick(&mut self, delta_ms: u32) -> bool {
        self.interaction_progress.set_target(self.interaction_target_progress());
        // Auto-repeat runs on this control's existing clock: no thread, no platform timer.
        let held = self.auto_repeat_held();
        let repeats = self.auto_repeat.tick(delta_ms, held);
        for _ in 0..repeats {
            self.emit_auto_repeat();
        }
        #[cfg_attr(not(full_widgets), allow(unused_mut))]
        let mut animating = self.interaction_progress.tick(delta_ms) || held;
        // A delayed popup is a held press, not an animation, but it still owes the bus a frame:
        // without that the timer would only fire when some other control happened to tick.
        #[cfg(full_widgets)]
        if let Some(elapsed) = self.press_elapsed.as_mut() {
            *elapsed = elapsed.saturating_add(delta_ms);
            if *elapsed >= self.popup_delay_ms {
                self.press_elapsed = None;
                self.open_menu();
            }
            animating = true;
        }
        animating
    }
}
impl Widget for ToggleButton {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    /// Keeps the exclusive group's geometry snapshot current.
    fn set_geometry(&mut self, geometry: Rect) {
        self.base.set_geometry(geometry);
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

    /// Keeps the group's enabled snapshot current, so a peer excludes it correctly.
    fn set_enabled(&mut self, enabled: bool) {
        if self.base.is_enabled() == enabled {
            return;
        }
        self.base.set_enabled(enabled);
        self.set_state_theme_hook();
        self.sync_group_registration();
        self.base.request_redraw();
    }

    /// The button and its open menu accept the pointer.
    fn contains_point(&self, point: Point) -> bool {
        #[cfg(full_widgets)]
        if self.menu_open && self.menu_rect().contains_point(point) {
            return true;
        }
        self.base.contains_point_with_touch_expansion(point)
    }

    #[cfg(full_widgets)]
    fn has_open_popup(&self) -> bool {
        self.menu_open
    }

    /// A right-click on a button with a menu opens it at the pointer.
    #[cfg(full_widgets)]
    fn context_menu_event(&mut self, pos: Point) -> bool {
        if self.menu_items.is_empty() {
            return false;
        }
        // The attached menu anchors below the button; the pointer position is the request's
        // anchor rather than a panel origin, so the menu is opened at its usual place. A caller
        // that wants a true context menu has the `Menu` control for it.
        let _ = pos;
        self.open_menu();
        true
    }

    fn size_hint(&self) -> Size {
        // The same `max(floor, content + padding)` formula `Button` uses, through the shared
        // primitive rather than the hand-written `max(75)` this used to spell out. The old
        // literal pair (`75`/`28`) disagreed with `Button`'s `BUTTON_MIN` (64x40) about how
        // big a push button is, so two buttons that look identical reported different sizes
        // and — once `draw` started deriving its band from this hint — drew at two heights.
        //
        // # Why the width comes from the shared estimate and not `len() * 8`
        //
        // This read `self.text().len() as u32 * 8`, which counts UTF-8 **bytes**, while
        // `Button::implicit_size` — the sibling this comment says it matches — measures with
        // `estimate_text_width`, which counts **clusters** and applies the font's `letter_spacing`.
        // The comment was therefore describing an agreement the code did not have: a `Button` and a
        // `ToggleButton` with the same caption reported different widths, and the gap widened with
        // every non-ASCII character (a CJK caption measured three times its drawn width).
        //
        // The answer is cached against its inputs for the same reason `Button::implicit_size` is:
        // `draw` asks for it on every frame, and a hover or interaction fade repaints many frames
        // with nothing about this hint changed.
        let font = effective_font(self.style());
        let key = ToggleHintKey {
            text_revision: self.text_revision,
            font: font.clone(),
            icon_advance: self.icon_advance(),
            arrow_advance: self.arrow_advance(),
        };
        let cached = (*self.hint_cache.borrow()).clone();
        if let Some((cached_key, size)) = cached {
            if cached_key == key {
                return size;
            }
        }
        let size = self.measure_hint(font, key.icon_advance, key.arrow_advance);
        *self.hint_cache.borrow_mut() = Some((key, size));
        size
    }

    /// The latched state, reported so the theme's `toggle_button:checked` override is reachable.
    ///
    /// # The defect this closes
    ///
    /// The trait's default `widget_state` knows only the four primitive flags
    /// (`disabled`/`pressed`/`hovered`/`focused`) and **none of them means "latched"**, so a checked
    /// toggle reported `Normal` and the theme's state lookup — `"<kind>:<state>"` — could not reach a
    /// checked key. `CheckBox` was fixed for exactly this reason (see
    /// `widget_state_reports_checked_for_a_latched_box`); `ToggleButton` was missed, and the symptom
    /// was measurable: its `_checked` extra appearance rendered **byte-identical** to the default,
    /// because `draw`'s own `if self.checked` branch was shadowed by the base fill the theme always
    /// supplies. A toggle with no visible difference between on and off is not a toggle.
    fn widget_state(&self) -> crate::style::WidgetState {
        use crate::style::WidgetState;
        if !self.base.is_enabled() {
            return WidgetState::Disabled;
        }
        // The latch outranks the momentary states, matching `CheckBox`: a checked toggle that is
        // hovered is still checked, and the checked fill is the fact a user is looking for.
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

    /// Lifts the control's own `tick` onto the trait so a host holding `&mut dyn Widget` can
    /// advance it. One line, and it is the whole reason the trait method exists: without it
    /// the animation is written and unreachable.
    fn tick(&mut self, delta_ms: u32) -> bool {
        ToggleButton::tick(self, delta_ms)
    }

    fn is_animating(&self) -> bool {
        // Derived from the *state*, not from the tick-time target field: a hover arriving makes
        // the control animating at once, before any `tick` has run to recompute the target.
        // Reading the stale field would answer `false` for a button that was just hovered, so the
        // bus would never start its frames. `Button::is_animating` documents the same trap.
        #[cfg(full_widgets)]
        if self.press_elapsed.is_some() {
            return true;
        }
        self.interaction_progress.value() != self.interaction_target_progress()
            || self.auto_repeat_held()
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();
    /// Resolves a published event name to the control's own signal.
    ///
    /// # Why the field is named per arm
    ///
    /// The published name and the Rust field name are not always the same (`find_next` is backed by
    /// `find_next_signal`, `dismissed` by a `Signal1<()>` field). `connect_event` validates a name
    /// against the capability table and registers a hub slot; only `event_signal_dyn` joins that
    /// name to the signal the control actually **emits**. A wrong arm is worse than no arm, because
    /// it reports a wire as live and never fires it, so each field is named explicitly here rather
    /// than derived from the published name.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "toggled" => Some(EventSignalRef::mapped("toggled", &self.toggled, |v| {
                CapabilityValue::Bool(*v)
            })),
            "checked_changed" => {
                Some(EventSignalRef::mapped("checked_changed", &self.checked_changed, |v| {
                    CapabilityValue::Bool(*v)
                }))
            }
            "pressed" => Some(EventSignalRef::unit("pressed", &self.pressed_signal)),
            "released" => Some(EventSignalRef::unit("released", &self.released_signal)),
            "state_changed" => {
                Some(EventSignalRef::mapped("state_changed", &self.state_changed, |v| {
                    CapabilityValue::String(format!("{v:?}"))
                }))
            }
            #[cfg(full_widgets)]
            "item_triggered" => {
                Some(EventSignalRef::mapped("item_triggered", &self.item_triggered, |v| {
                    crate::widget::capability::CapabilityValue::UInt(*v)
                }))
            }
            _ => None,
        }
    }
}

/// `ToggleButton`'s property contract.
///
/// `state` is the derived interaction state, in the order `state()` resolves it:
/// `"disabled"` (the control is off), `"checked"` (a latch — a value, not a gesture),
/// `"pressed"` (held), `"hover"` (the pointer is over it), `"normal"`.
///
/// It is read-only, which is why it has a read arm but no write arm here — exactly as the
/// previous centralised dispatch behaved. The published vocabulary is all five of those words:
/// an earlier revision listed only `normal`/`checked`/`disabled` and so quoted a set the reader
/// leaves as soon as the pointer arrives.
impl WidgetProperties for ToggleButton {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "text" => Ok(CapabilityValue::String(self.text().to_string())),
            "checked" => Ok(CapabilityValue::Bool(self.is_checked())),
            "state" => {
                let state = match self.state() {
                    ToggleButtonState::Normal => "normal",
                    ToggleButtonState::Hover => "hover",
                    ToggleButtonState::Pressed => "pressed",
                    ToggleButtonState::Checked => "checked",
                    ToggleButtonState::Disabled => "disabled",
                };
                Ok(CapabilityValue::String(state.to_string()))
            }
            "auto_exclusive" => Ok(CapabilityValue::Bool(self.is_auto_exclusive())),
            "auto_raise" => Ok(CapabilityValue::Bool(self.is_auto_raise())),
            "auto_repeat" => Ok(CapabilityValue::Bool(self.is_auto_repeat())),
            "auto_repeat_delay" => Ok(CapabilityValue::UInt(self.auto_repeat_delay() as u64)),
            "auto_repeat_interval" => Ok(CapabilityValue::UInt(self.auto_repeat_interval() as u64)),
            "group_id" => match self.group_id() {
                Some(id) => Ok(CapabilityValue::String(id.to_string())),
                None => Ok(CapabilityValue::Null),
            },
            "icon_source" => match self.icon_source() {
                None => Ok(CapabilityValue::Null),
                Some(IconSource::Builtin(name)) => Ok(CapabilityValue::IconSpec(
                    crate::widget::icon_source::IconSpec::Builtin(name),
                )),
                Some(IconSource::StaticGeometry(_)) => {
                    Err(CapabilityAccessError::IconSourceNotPersistable)
                }
            },
            #[cfg(full_widgets)]
            "arrow_type" => Ok(CapabilityValue::String(
                match self.arrow_type {
                    MenuArrowType::None => "none",
                    MenuArrowType::Down => "down",
                    MenuArrowType::Up => "up",
                    MenuArrowType::Left => "left",
                    MenuArrowType::Right => "right",
                }
                .to_string(),
            )),
            #[cfg(full_widgets)]
            "popup_mode" => Ok(CapabilityValue::String(
                match self.popup_mode {
                    MenuButtonPopup::InstantPopup => "instant",
                    MenuButtonPopup::MenuButtonPopup => "menu_button",
                    MenuButtonPopup::DelayedPopup => "delayed",
                }
                .to_string(),
            )),
            #[cfg(full_widgets)]
            "menu_open" => Ok(CapabilityValue::Bool(self.is_menu_open())),
            #[cfg(full_widgets)]
            "menu_item_count" => Ok(CapabilityValue::UInt(self.menu_items.len() as u64)),
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
            "state" => Err(CapabilityAccessError::ReadOnlyProperty),
            "auto_exclusive" => {
                self.set_auto_exclusive(expect_bool(value)?);
                Ok(())
            }
            "auto_raise" => {
                self.set_auto_raise(expect_bool(value)?);
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
            "group_id" => {
                match value {
                    CapabilityValue::Null => self.set_group_id(None),
                    other => self.set_group_id(Some(expect_string(other)?)),
                }
                Ok(())
            }
            "icon_source" => {
                match value {
                    CapabilityValue::Null => self.set_icon_source(None),
                    CapabilityValue::IconSpec(crate::widget::icon_source::IconSpec::Builtin(
                        name,
                    )) => self.set_icon_source(Some(IconSource::Builtin(name))),
                    CapabilityValue::IconSpec(crate::widget::icon_source::IconSpec::Resource {
                        ..
                    }) => return Err(CapabilityAccessError::ResourceResolutionRequired),
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                }
                Ok(())
            }
            #[cfg(full_widgets)]
            "arrow_type" => {
                let token = expect_string(value)?;
                let arrow = match token.as_str() {
                    "none" => MenuArrowType::None,
                    "down" => MenuArrowType::Down,
                    "up" => MenuArrowType::Up,
                    "left" => MenuArrowType::Left,
                    "right" => MenuArrowType::Right,
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                self.set_arrow_type(arrow);
                Ok(())
            }
            #[cfg(full_widgets)]
            "popup_mode" => {
                let token = expect_string(value)?;
                let mode = match token.as_str() {
                    "instant" => MenuButtonPopup::InstantPopup,
                    "menu_button" => MenuButtonPopup::MenuButtonPopup,
                    "delayed" => MenuButtonPopup::DelayedPopup,
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                self.set_popup_mode(mode);
                Ok(())
            }
            #[cfg(full_widgets)]
            "menu_open" => {
                if expect_bool(value)? {
                    self.open_menu();
                } else {
                    self.close_menu();
                }
                Ok(())
            }
            #[cfg(full_widgets)]
            "menu_item_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn set_resolved_icon_source(
        &mut self,
        source: Option<IconSource>,
    ) -> Result<(), CapabilityAccessError> {
        self.set_icon_source(source);
        Ok(())
    }

    fn property_names(&self) -> &'static [&'static str] {
        #[cfg(full_widgets)]
        return property_names_of![
            "text",
            "checked",
            "state",
            "auto_exclusive",
            "auto_raise",
            "auto_repeat",
            "auto_repeat_delay",
            "auto_repeat_interval",
            "group_id",
            "icon_source",
            "arrow_type",
            "popup_mode",
            "menu_open",
            "menu_item_count",
            BASE_PROPERTY_NAMES
        ];
        #[cfg(not(full_widgets))]
        return property_names_of![
            "text",
            "checked",
            "state",
            "auto_exclusive",
            "auto_raise",
            "auto_repeat",
            "auto_repeat_delay",
            "auto_repeat_interval",
            "group_id",
            "icon_source",
            BASE_PROPERTY_NAMES
        ];
    }

    /// Runs one of the commands `toggle_button` publishes.
    ///
    /// `toggle` flips the latch. `set_checked` and `set_text` assign state and need
    /// an argument, so they are answered through the property path and refused here
    /// as [`CapabilityAccessError::OutOfRange`] — "the name is right, the invocation
    /// needs a payload" — rather than `UnknownCommand`.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "toggle" => {
                self.toggle();
                Ok(())
            }
            "set_checked" | "set_text" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl Draw for ToggleButton {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.base.geometry();
        let state = self.state();
        let style = self.style();
        use crate::core::Color;
        // Read the three role colours out and **release the guard** before drawing. `theme_manager()`
        // returns a `MutexGuard`, and the accessors below (`layer_color`, `semantic_color`) take the
        // same non-reentrant lock themselves — so holding it here and calling one of them deadlocks.
        // The crate's own doc says a guard must not be held across a draw; this is that rule.
        let (outline, outline_variant, disabled) = {
            let manager = crate::style::theme_manager();
            match manager.current_theme() {
                Some(active) => (
                    Some(active.colors.outline),
                    Some(active.colors.outline_variant),
                    Some(active.colors.disabled),
                ),
                None => (None, None, None),
            }
        };

        // ── The box actually painted ──
        //
        // The toggle fills the width it was *given*, exactly as `Button` does, but its height
        // is its own. Painting `rect` directly drew a 240x120 census cell as a 240x120 slab
        // with the caption floating in the middle of it — a rectangle shaped like a button
        // rather than a button, and visibly taller than the `Button` beside it. The height
        // comes from `size_hint`, the same derivation a layout asks for, so the drawn shape
        // and the reported size cannot disagree.
        let rect = ControlMetrics::full_width_band(rect, self.size_hint().height);

        // ── Background ──
        //
        // Three literals used to decide this, and the enum had no hover or pressed arm at all, so
        // a toggle button gave **no feedback until the latch flipped** — the one moment feedback
        // was least needed. The resting colours now come from the theme like every other control's
        // (BLUE23 §5 / M4), and the interaction is a **blend** whose weight is the transition's
        // progress, so a hover fades in and a press deepens it rather than both snapping. At
        // progress `0.0` the result is exactly the resting colour, which is why every committed
        // snapshot stayed byte-identical.
        let resting = style.background_color.unwrap_or_else(|| {
            // Both resting colours are **theme roles** rather than the literals that used to be
            // here (`rgb(200,220,255)` checked, `rgb(240,240,240)` normal). The checked one was
            // the light preset's tint, so a checked toggle read wrong on every dark appearance;
            // the roles move with the appearance by construction.
            let role = if self.is_checked() {
                crate::style::LayerColor::SurfaceContainerHigh
            } else {
                crate::style::LayerColor::SurfaceContainer
            };
            crate::style::layer_color(role).unwrap_or(if self.is_checked() {
                Color::rgb(200, 220, 255)
            } else {
                Color::rgb(240, 240, 240)
            })
        });
        let bg_color = if state == ToggleButtonState::Disabled {
            resting
        } else {
            let progress = self.interaction_progress.value();
            if progress <= 0.0 {
                resting
            } else {
                let interactive = if state == ToggleButtonState::Pressed {
                    resting
                } else {
                    resting.blend(&resting.contrast_color(), 0.22)
                };
                resting.blend(&interactive, progress)
            }
        };
        // An auto-raising tool button shows no chrome at rest. The state it needs is already
        // derived: `Checked` is persistent and the two momentary states cover hover and press.
        let chrome = !self.auto_raise
            || self.is_checked()
            || self.base.is_hovered()
            || self.base.is_pressed();
        if chrome {
            context.face_with_gradient(
                rect,
                bg_color,
                self.style().background_gradient.as_ref(),
                self.style().surface.unwrap_or_default(),
                self.style().border_radius.unwrap_or(0),
                Color::BLACK,
            );
        }

        // ── Border ──
        //
        // The checked border is the theme's **info** token rather than the literal
        // `rgb(80, 120, 200)`, for the same reason the fill above is: a control whose "on" colour
        // does not move with the appearance is theme-blind in exactly the way the census exists to
        // catch. The unchecked border reads the theme's separator role — the weak one when
        // disabled, so "switched off" and "inert" are two different lines rather than the same
        // grey.
        let border_color = style.border_color.unwrap_or_else(|| {
            if self.is_checked() {
                crate::style::semantic_color(crate::style::SemanticColor::Info)
                    .unwrap_or(Color::rgb(80, 120, 200))
            } else if state == ToggleButtonState::Disabled {
                outline_variant.unwrap_or(Color::rgb(180, 180, 180))
            } else {
                outline.unwrap_or(Color::rgb(180, 180, 180))
            }
        });
        let bw = style.border_width.unwrap_or(0);
        let border_width = if bw > 0 { bw } else { 1 };
        if chrome {
            context.draw_rect_stroke(rect, border_color, border_width);
        }

        // ── Icon, arrow and text ──
        let font = effective_font(style);
        let text_color = style.text_color.unwrap_or_else(|| {
            if state == ToggleButtonState::Disabled {
                // The disabled ink is the theme's own disabled colour rather than the
                // near-invisible `rgb(150, 150, 150)` literal, which measured below the
                // 4.5:1 floor once the disabled fill went to the theme's surface.
                disabled.unwrap_or(Color::rgb(150, 150, 150))
            } else {
                // The label sits on the control's own fill, so it takes that fill's contrast
                // colour instead of a literal black — the same rule `CheckBox`'s mark, the
                // `group_box` tick and this crate's other indicators follow.
                bg_color.contrast_color()
            }
        });
        let line = context.text_line(rect, font);
        let mut leading = dimensions::BUTTON_PADDING_H as i32;
        if let Some(source) = self.icon_source {
            // The icon shares the text's line box, so a bigger font moves both by the same
            // amount rather than leaving the icon floating on the button's midpoint.
            let side = dimensions::TOOL_BUTTON_ICON_SIZE.min(rect.height).min(line.height.max(1));
            let icon_rect = Rect::new(rect.x + leading, line.y, side, side);
            if let Err(error) =
                crate::widget::paint_icon(context, source.into(), icon_rect, text_color)
            {
                log::warn!("tool button icon could not be painted: {error:?}");
            }
            leading += side as i32 + dimensions::TOOL_BUTTON_ICON_SPACING as i32;
        }
        #[cfg_attr(not(full_widgets), allow(unused_mut))]
        let mut trailing = dimensions::BUTTON_PADDING_H as i32;
        #[cfg(full_widgets)]
        if self.arrow_type != MenuArrowType::None {
            trailing += 12;
            let arrow_x = rect.x + rect.width as i32 - trailing;
            let arrow_y = rect.y + rect.height as i32 / 2 - 2;
            // Constant vertex offsets rather than a freshly built `Vec` per frame. The
            // arrow's shape never varies, so the vertices are a stack array the draw path
            // borrows (G-A08).
            let offsets: [(i32, i32); 3] = match self.arrow_type {
                MenuArrowType::Up => [(0, 6), (8, 6), (4, 0)],
                MenuArrowType::Left => [(6, 0), (6, 6), (0, 3)],
                MenuArrowType::Right => [(0, 0), (0, 6), (6, 3)],
                _ => [(0, 0), (8, 0), (4, 6)],
            };
            let points: [crate::core::Point; 3] =
                offsets.map(|(dx, dy)| crate::core::Point::new(arrow_x + dx, arrow_y + dy));
            context.draw_path(&points, true, text_color, true, 1);
        }
        if !self.text.is_empty() {
            let band_x = rect.x + leading;
            let band_width = (rect.width as i32 - leading - trailing).max(0) as u32;
            let text_band = Rect::new(band_x, line.y, band_width, line.height);
            context.draw_text_fitted_cow(
                text_band,
                &self.text,
                font,
                text_color,
                HorizontalAlignment::Center,
            );
        }

        // ── Attached menu ──
        #[cfg(full_widgets)]
        if self.menu_open {
            let panel = self.menu_rect();
            let accent = crate::style::palette_shared()
                .as_ref()
                .map(|theme| theme.accent)
                .unwrap_or_else(|| bg_color.contrast_color());
            draw_menu_panel(context, panel, &self.menu_items, &[], style, accent);
            if let Some(index) = self.active_menu_index {
                if self.menu_items.get(index).is_some_and(|item| item.enabled) {
                    let row = menu_item_rect(panel, index);
                    context.draw_rounded_rect_stroke(row, 2, accent, 1);
                }
            }
        }
    }
}
/// A dropped tool button leaves its exclusive group.
impl Drop for ToggleButton {
    fn drop(&mut self) {
        button_group::leave_all(self.id());
    }
}

impl crate::event::EventHandler for ToggleButton {
    /// Toggles on a completed activation, mirroring `Button`.
    ///
    /// # Why the hit test and the leave arm are load-bearing
    ///
    /// The press arm discarded the position (`pos: _`) and there was no `MouseLeave` arm, so a
    /// press **anywhere** set `pressed = true` and nothing ever cleared it if the pointer left
    /// without a release reaching the control. The latch then committed on an unrelated later
    /// release, and `draw` kept painting the pressed state meanwhile. `Button` guards the release
    /// on `self.pressed`; this control now does the same and arms only for a press that lands on it.
    fn handle_event(&mut self, event: &crate::event::Event) {
        self.base.handle_event(event);
        self.sync_group_registration();
        if !self.base.is_enabled() {
            return;
        }

        // An open menu owns the next interaction.
        #[cfg(full_widgets)]
        if self.menu_open {
            let mut consumed = false;
            match event {
                crate::event::Event::KeyPress { key, .. }
                    if *key == crate::event::key_codes::ESCAPE =>
                {
                    self.close_menu();
                    consumed = true;
                }
                crate::event::Event::KeyPress { key, .. }
                    if *key == crate::event::key_codes::DOWN =>
                {
                    self.move_menu_highlight(true);
                    consumed = true;
                }
                crate::event::Event::KeyPress { key, .. }
                    if *key == crate::event::key_codes::UP =>
                {
                    self.move_menu_highlight(false);
                    consumed = true;
                }
                crate::event::Event::KeyPress { key, .. }
                    if *key == crate::event::key_codes::ENTER
                        || *key == crate::event::key_codes::SPACE =>
                {
                    if let Some(index) = self.active_menu_index {
                        self.activate_menu_row(index);
                    }
                    consumed = true;
                }
                crate::event::Event::MousePress { pos, button, .. } if *button == 1 => {
                    if let Some(index) = self.hit_test_menu_item(*pos) {
                        self.activate_menu_row(index);
                    } else if !self.menu_rect().contains_point(*pos) {
                        self.close_menu();
                    }
                    consumed = true;
                }
                crate::event::Event::MouseMove { pos } => {
                    if let Some(index) = self.hit_test_menu_item(*pos) {
                        if self.active_menu_index != Some(index) {
                            self.active_menu_index = Some(index);
                            self.base.request_redraw();
                        }
                    }
                }
                _ => { /* Other events reach the ordinary arms below */ }
            }
            if consumed {
                return;
            }
        }

        match event {
            crate::event::Event::MousePress { pos, button, .. } if *button == 1 => {
                #[cfg(full_widgets)]
                if !self.menu_items.is_empty() {
                    let on_arrow = self.arrow_rect().contains_point(*pos)
                        || self.arrow_type == MenuArrowType::None;
                    match self.popup_mode {
                        MenuButtonPopup::InstantPopup => {
                            self.toggle_menu();
                            return;
                        }
                        MenuButtonPopup::MenuButtonPopup if on_arrow => {
                            self.toggle_menu();
                            return;
                        }
                        MenuButtonPopup::DelayedPopup if on_arrow => {
                            self.open_menu();
                            return;
                        }
                        MenuButtonPopup::DelayedPopup => {
                            // Start the hold timer; `tick` opens the menu when it fires.
                            self.press_elapsed = Some(0);
                            self.base.request_redraw();
                            return;
                        }
                        MenuButtonPopup::MenuButtonPopup => { /* the face still toggles */ }
                    }
                }
                // Arm only for a press on the control; a press outside must not leave the latch
                // set for a later release.
                self.set_pressed(self.base.contains_point_with_touch_expansion(*pos));
            }
            crate::event::Event::MouseRelease { pos, button } if *button == 1 => {
                #[cfg(full_widgets)]
                {
                    // A release before the delay fired cancels the pending popup.
                    self.press_elapsed = None;
                }
                // Commit only a press that this control armed, and only while the pointer is
                // still over it — the same two conditions `Button` applies. The guard reads
                // `signaled_pressed`, not the base flag: the base clears `pressed` on the
                // release before this arm runs, so the paint flag can no longer say
                // whether this control had armed the gesture.
                if self.signaled_pressed && self.base.contains_point_with_touch_expansion(*pos) {
                    self.toggle();
                }
                self.set_pressed(false);
            }
            // (The former blanket `MouseRelease { button: 1, .. }` arm here was unreachable: it
            // matched a strict subset of the arm above, so it could never run — G-A08.)
            crate::event::Event::MouseLeave { .. } => {
                // Abandon a held press; without arms this control had no leave handling at all.
                #[cfg(full_widgets)]
                {
                    self.press_elapsed = None;
                }
                self.set_pressed(false);
            }
            crate::event::Event::FocusLost => {
                self.set_pressed(false);
            }
            // Space and Enter activate a tool button, matching the push button's contract.
            crate::event::Event::KeyPress { key, .. }
                if *key == crate::event::key_codes::SPACE
                    || *key == crate::event::key_codes::ENTER
                    || *key == crate::event::key_codes::LINE_FEED =>
            {
                self.toggle();
                self.emit_clicked();
            }
            _ => { /* Other events are not relevant */ }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Color, ObjectId, Point, Rect};
    use crate::event::{Event, EventHandler};
    use crate::style::WidgetStyle;

    #[test]
    fn toggle_creation_defaults() {
        let tb = ToggleButton::new("Toggle".to_string(), Rect::new(0, 0, 100, 30));
        assert!(!tb.is_checked());
        assert_eq!(tb.text(), "Toggle");
        assert!(!tb.is_auto_exclusive());
        assert!(tb.group_id().is_none());
        assert!(!tb.is_pressed());
        assert_eq!(tb.state(), ToggleButtonState::Normal);
    }

    #[test]
    fn toggle_set_checked() {
        let mut tb = ToggleButton::new("T".to_string(), Rect::new(0, 0, 50, 30));
        tb.set_checked(true);
        assert!(tb.is_checked());
        assert_eq!(tb.state(), ToggleButtonState::Checked);
        tb.set_checked(false);
        assert!(!tb.is_checked());
        assert_eq!(tb.state(), ToggleButtonState::Normal);
    }

    #[test]
    fn toggle_toggle_method() {
        let mut tb = ToggleButton::new("T".to_string(), Rect::new(0, 0, 50, 30));
        assert!(!tb.is_checked());
        tb.toggle();
        assert!(tb.is_checked());
        tb.toggle();
        assert!(!tb.is_checked());
    }

    #[test]
    fn toggle_set_text() {
        let mut tb = ToggleButton::new("Old".to_string(), Rect::new(0, 0, 100, 30));
        assert_eq!(tb.text(), "Old");
        tb.set_text("New".to_string());
        assert_eq!(tb.text(), "New");
    }

    #[test]
    fn toggle_auto_exclusive() {
        let mut tb = ToggleButton::new("T".to_string(), Rect::new(0, 0, 50, 30));
        assert!(!tb.is_auto_exclusive());
        tb.set_auto_exclusive(true);
        assert!(tb.is_auto_exclusive());
        tb.set_auto_exclusive(false);
        assert!(!tb.is_auto_exclusive());
    }

    #[test]
    fn toggle_group_id() {
        let mut tb = ToggleButton::new("T".to_string(), Rect::new(0, 0, 50, 30));
        assert!(tb.group_id().is_none());
        tb.set_group_id(Some("group1".to_string()));
        assert_eq!(tb.group_id(), Some("group1"));
        tb.set_group_id(None);
        assert!(tb.group_id().is_none());
    }

    #[test]
    fn toggle_pressed_state() {
        let mut tb = ToggleButton::new("T".to_string(), Rect::new(0, 0, 50, 30));
        assert!(!tb.is_pressed());
        tb.set_pressed(true);
        assert!(tb.is_pressed());
        tb.set_pressed(false);
        assert!(!tb.is_pressed());
    }

    #[test]
    fn toggle_geometry_delegation() {
        let mut tb = ToggleButton::new("T".to_string(), Rect::new(0, 0, 100, 30));
        tb.set_geometry(Rect::new(10, 10, 200, 50));
        assert_eq!(tb.geometry(), Rect::new(10, 10, 200, 50));
    }

    #[test]
    fn toggle_visibility_delegation() {
        let mut tb = ToggleButton::new("T".to_string(), Rect::new(0, 0, 100, 30));
        assert!(tb.is_visible());
        tb.hide();
        assert!(!tb.is_visible());
        tb.show();
        assert!(tb.is_visible());
    }

    #[test]
    fn toggle_enabled_delegation() {
        let mut tb = ToggleButton::new("T".to_string(), Rect::new(0, 0, 100, 30));
        assert!(tb.is_enabled());
        tb.set_enabled(false);
        assert!(!tb.is_enabled());
        assert_eq!(tb.state(), ToggleButtonState::Disabled);
        tb.set_enabled(true);
        assert!(tb.is_enabled());
    }

    #[test]
    fn toggle_parent_children() {
        let mut tb = ToggleButton::new("T".to_string(), Rect::new(0, 0, 100, 30));
        assert!(tb.parent().is_none());
        let pid: ObjectId = 42;
        tb.set_parent(Some(pid));
        assert_eq!(tb.parent(), Some(pid));
        tb.set_parent(None);
        assert!(tb.parent().is_none());

        let cid: ObjectId = 100;
        tb.add_child(cid);
        assert_eq!(tb.children().len(), 1);
        assert_eq!(tb.children()[0], cid);
        tb.remove_child(cid);
        assert!(tb.children().is_empty());
    }

    #[test]
    fn toggle_tooltip_roundtrip() {
        let mut tb = ToggleButton::new("T".to_string(), Rect::new(0, 0, 100, 30));
        assert!(tb.tooltip().is_empty());
        tb.set_tooltip("Helpful tip".to_string());
        assert_eq!(tb.tooltip(), "Helpful tip");
        tb.set_tooltip(String::new());
        assert!(tb.tooltip().is_empty());
    }

    #[test]
    fn toggle_style_roundtrip() {
        let mut tb = ToggleButton::new("T".to_string(), Rect::new(0, 0, 100, 30));
        assert_eq!(*tb.style(), WidgetStyle::default());
        let custom = WidgetStyle::default().with_background(Color::rgb(200, 200, 200));
        tb.set_style(custom.clone());
        assert_eq!(*tb.style(), custom);
    }

    #[test]
    fn toggle_id_kind() {
        let tb_a = ToggleButton::new("A".to_string(), Rect::new(0, 0, 50, 30));
        let tb_b = ToggleButton::new("B".to_string(), Rect::new(0, 0, 50, 30));
        assert_ne!(tb_a.id(), tb_b.id());
        assert_eq!(tb_a.kind(), WidgetKind::ToggleButton);
        assert_eq!(tb_b.kind(), WidgetKind::ToggleButton);
    }

    #[test]
    fn toggle_signal_accessors() {
        let tb = ToggleButton::new("T".to_string(), Rect::new(0, 0, 50, 30));
        // Signal1<bool>
        let _toggled = &tb.toggled;
        let _checked = &tb.checked_changed;
        let _state = &tb.state_changed;
        // GenericSignal
        let _pressed = &tb.pressed_signal;
        let _released = &tb.released_signal;
    }
    /// A press that leaves the control is abandoned, and never latches.
    ///
    /// The press arm discarded the position and there was no `MouseLeave` arm, so a press
    /// anywhere set `pressed = true` and only a release cleared it. A pointer that left while
    /// held left the latch set, and the widget then toggled on an unrelated later release.
    #[test]
    fn toggle_button_mouse_leave_abandons_a_held_press() {
        let inside = Point::new(20, 15);
        let mut tb = ToggleButton::new("T".to_string(), Rect::new(0, 0, 100, 30));

        tb.handle_event(&Event::MousePress { pos: inside, button: 1, modifiers: 0 });
        assert!(tb.is_pressed(), "a press on the control arms the latch");

        tb.handle_event(&Event::MouseLeave { pos: Point::new(-1, -1) });
        assert!(!tb.is_pressed(), "leaving must clear the latch");

        // The stray release must not commit, and the latch must already be clear.
        tb.handle_event(&Event::MouseRelease { pos: inside, button: 1 });
        assert!(!tb.is_checked(), "a release after leaving must not toggle");
    }

    /// A press that does not land on the control must not arm the latch.
    #[test]
    fn toggle_button_press_outside_does_not_arm() {
        let mut tb = ToggleButton::new("T".to_string(), Rect::new(0, 0, 100, 30));
        tb.handle_event(&Event::MousePress {
            pos: Point::new(9000, 9000),
            button: 1,
            modifiers: 0,
        });
        assert!(!tb.is_pressed());
        tb.handle_event(&Event::MouseRelease { pos: Point::new(20, 15), button: 1 });
        assert!(!tb.is_checked(), "a drag that began outside must not commit");
    }

    /// A completed activation still toggles.
    #[test]
    fn toggle_button_completed_activation_toggles() {
        let inside = Point::new(20, 15);
        let mut tb = ToggleButton::new("T".to_string(), Rect::new(0, 0, 100, 30));
        tb.handle_event(&Event::MousePress { pos: inside, button: 1, modifiers: 0 });
        tb.handle_event(&Event::MouseRelease { pos: inside, button: 1 });
        assert!(tb.is_checked());
    }

    /// Losing focus abandons a held press.
    #[test]
    fn toggle_button_focus_loss_abandons_a_held_press() {
        let inside = Point::new(20, 15);
        let mut tb = ToggleButton::new("T".to_string(), Rect::new(0, 0, 100, 30));
        tb.handle_event(&Event::MousePress { pos: inside, button: 1, modifiers: 0 });
        tb.handle_event(&Event::FocusLost);
        assert!(!tb.is_pressed());
        tb.handle_event(&Event::MouseRelease { pos: inside, button: 1 });
        assert!(!tb.is_checked());
    }

    /// BLUE23 Appendix A.2 / M1: the two *momentary* states are now reachable.
    ///
    /// # The defect this pins
    ///
    /// `ToggleButtonState` had exactly `Normal` / `Checked` / `Disabled`. The control maintained
    /// `is_pressed` and this crate's `BaseWidget` maintains `hovered`, and **neither reached the
    /// enum**, so `draw` could not express either. A user pointing at a toggle button got no
    /// feedback at all until the latch flipped — the one moment feedback was least needed.
    #[test]
    fn a_momentary_state_is_reachable_on_a_toggle_button() {
        let inside = Point::new(20, 15);
        let mut tb = ToggleButton::new("T".to_string(), Rect::new(0, 0, 100, 30));
        assert_eq!(tb.state(), ToggleButtonState::Normal);

        tb.handle_event(&Event::MouseEnter { pos: inside });
        assert_eq!(tb.state(), ToggleButtonState::Hover, "a pointer over the control is a state");

        tb.handle_event(&Event::MousePress { pos: inside, button: 1, modifiers: 0 });
        assert_eq!(tb.state(), ToggleButtonState::Pressed, "a held control is a state");

        tb.handle_event(&Event::MouseRelease { pos: inside, button: 1 });
        // The latch wins once it is set: "checked" is a persistent fact, and the hover it
        // happens to sit under is the *overlay* the draw path adds. Asserting the precedence
        // stops someone "fixing" the enum order later and silently breaking the property value.
        assert_eq!(tb.state(), ToggleButtonState::Checked);

        tb.handle_event(&Event::MouseLeave { pos: inside });
        tb.set_checked(false);
        assert_eq!(tb.state(), ToggleButtonState::Normal);

        tb.set_enabled(false);
        assert_eq!(
            tb.state(),
            ToggleButtonState::Disabled,
            "disabled outranks a momentary state, exactly as it outranks checked"
        );
    }

    /// The state the property path publishes has to name the new arms, or the two views of the
    /// enum disagree (BLUE23 M10: "declared but cannot be read back").
    #[test]
    fn the_state_property_names_every_arm() {
        let inside = Point::new(20, 15);
        let mut tb = ToggleButton::new("T".to_string(), Rect::new(0, 0, 100, 30));
        let read = |w: &ToggleButton| match w.get("state") {
            Ok(CapabilityValue::String(s)) => s,
            other => panic!("state must read back as a string, got {other:?}"),
        };
        assert_eq!(read(&tb), "normal");
        tb.handle_event(&Event::MouseEnter { pos: inside });
        assert_eq!(read(&tb), "hover");
        tb.handle_event(&Event::MousePress { pos: inside, button: 1, modifiers: 0 });
        assert_eq!(read(&tb), "pressed");
    }

    /// BLUE23 Appendix A.2 / M3 + §9 Criterion 10: three frames of the interaction are **geometrically
    /// distinct**, and the fill arrives at the resting colour when the transition is at rest.
    ///
    /// The plan asks for "three geometrically distinct frames" on a button. A toggle button's shape does not move, so the
    /// observable is the **fill**: at `t=0` it is the resting colour, mid-transition it is between
    /// the resting and interactive colours, and at the end it has settled on the interactive one.
    ///
    /// # Why this is gated with `full_widgets`
    ///
    /// It installs the preset appearances through `widget::census`, which is itself
    /// `#[cfg(full_widgets)]` — and so is `crate::theme`. On a stripped target (an `ohos`/
    /// `android` build, where `full_widgets` is off) naming either one failed to *compile*:
    /// `cannot find `census` in `widget`` / `cannot find `theme` in `crate``. The test asserts a
    /// property of an installed palette, so a build that has no palette module cannot run it and
    /// should not try; the gate says exactly that.
    #[cfg(full_widgets)]
    #[test]
    fn the_interaction_transition_moves_the_fill_across_three_frames() {
        let _guard = crate::style::theme_test_guard();
        // The presets carry the `toggle_button:hover` / `:pressed` overrides, so a build with no
        // theme would have nothing to move the fill *toward* — and this test would be asserting a
        // property of a palette that is not installed.
        crate::widget::census::install_preset_appearances();
        let inside = Point::new(20, 15);
        let mut tb = ToggleButton::new("T".to_string(), Rect::new(0, 0, 100, 30));

        // At rest the control owes no frames, so it costs nothing per frame (judgement 8).
        assert!(!tb.is_animating(), "a resting toggle must not ask for frames");
        let resting = rendered_fill(&mut tb);

        // Hovering makes it animating *before* any tick has run — the trap `Button` documents:
        // reading the tick-time target field instead of the state answers `false` here and the
        // bus would never start.
        tb.handle_event(&Event::MouseEnter { pos: inside });
        assert!(tb.is_animating(), "a hovered toggle owes frames at once");

        // Frame 1: progress is still 0, so the fill is the resting colour exactly. This is also
        // why adding the transition left every committed snapshot byte-identical.
        assert_eq!(rendered_fill(&mut tb), resting, "the first frame is the resting colour");

        // The theme's own tempo, so a test can shorten it rather than wait. A one-millisecond step
        // would never land *exactly* on the target (`tick` answers `false` only within
        // `f32::EPSILON`), so the step is a whole frame's worth — the same 1000 ms `Button`'s own
        // transition tests use — and the loop is bounded besides.
        let mut mid = resting;
        for _ in 0..64 {
            if !tb.tick(16) {
                break;
            }
            let now = rendered_fill(&mut tb);
            if now != resting {
                mid = now;
                break;
            }
        }
        assert_ne!(mid, resting, "the transition must move the fill, not snap at the end");

        // Frame 3: run it out, and require it to settle *and* to stop asking for frames — the
        // economy `is_animating` exists for (a settled control costs nothing per frame). The bound
        // is on the loop itself, so a `tick` that never reports rest fails the test rather than
        // hanging it — the failure mode the first draft of this test actually had.
        let mut guard = 64;
        while guard > 0 && tb.tick(1000) {
            guard -= 1;
        }
        assert!(guard > 0, "the transition must terminate, not ask for frames forever");
        assert!(!tb.is_animating(), "a settled toggle must stop asking for frames");
        let settled = rendered_fill(&mut tb);
        assert_ne!(settled, resting, "a hovered toggle must not look like a resting one");
        assert_ne!(settled, mid, "the end state is reached, not abandoned mid-transition");
    }

    /// The fill the control actually paints, read out of the emitted SVG.
    ///
    /// Reading the drawing rather than the progress field is what makes the frame assertions
    /// properties of the picture: a mutation that stopped *using* `interaction_progress` in
    /// `draw` would leave every progress assertion green.
    ///
    /// # Why the *second* filled rect
    ///
    /// The exporter composites a control over the active theme's background, so the first
    /// `fill="rgba(` in the document is the **backdrop**, not the control. Reading it made this
    /// helper answer a constant white for every widget, which would have made the frame assertions
    /// compare a value against itself — the same trap the `group_box` tick test documents.
    #[cfg(full_widgets)]
    fn rendered_fill(tb: &mut ToggleButton) -> (u8, u8, u8) {
        let svg = crate::widget::svg::render_widget_to_svg(tb, Rect::new(0, 0, 100, 30));
        let key = "fill=\"rgba(";
        let backdrop = svg.find(key).expect("a backdrop rect") + key.len();
        let face =
            svg[backdrop..].find(key).expect("the control's own face") + backdrop + key.len();
        let end = svg[face..].find(')').expect("the fill's close") + face;
        let mut parts = svg[face..end].split(',');
        let r = parts.next().and_then(|v| v.trim().parse().ok()).expect("r");
        let g = parts.next().and_then(|v| v.trim().parse().ok()).expect("g");
        let b = parts.next().and_then(|v| v.trim().parse().ok()).expect("b");
        (r, g, b)
    }

    // ── Keyboard activation and exclusivity (R019–R021) ───────────────

    #[test]
    fn space_and_enter_toggle_the_button() {
        use crate::event::key_codes;
        let mut button = ToggleButton::new("Bold".to_string(), Rect::new(0, 0, 80, 30));
        button.handle_event(&Event::KeyPress { key: key_codes::SPACE, modifiers: 0 });
        assert!(button.is_checked(), "Space activates a focused tool button");
        button.handle_event(&Event::KeyPress { key: key_codes::ENTER, modifiers: 0 });
        assert!(!button.is_checked(), "Enter activates it too");
    }

    #[test]
    fn an_unrelated_key_does_not_toggle() {
        let mut button = ToggleButton::new("Bold".to_string(), Rect::new(0, 0, 80, 30));
        button.handle_event(&Event::KeyPress { key: 65, modifiers: 0 });
        assert!(!button.is_checked());
    }

    #[test]
    fn auto_exclusive_buttons_in_one_group_exclude_each_other() {
        let mut first = ToggleButton::new("Left".to_string(), Rect::new(0, 0, 60, 30));
        let mut second = ToggleButton::new("Right".to_string(), Rect::new(70, 0, 60, 30));
        first.set_group_id(Some("align".into()));
        second.set_group_id(Some("align".into()));
        first.set_auto_exclusive(true);
        second.set_auto_exclusive(true);

        first.set_checked(true);
        assert!(first.is_checked());
        second.set_checked(true);
        assert!(second.is_checked());
        assert!(!first.is_checked(), "the group holds one latch");
    }

    #[test]
    fn auto_exclusive_off_leaves_the_latches_independent() {
        let mut first = ToggleButton::new("Left".to_string(), Rect::new(0, 0, 60, 30));
        let mut second = ToggleButton::new("Right".to_string(), Rect::new(70, 0, 60, 30));
        first.set_group_id(Some("align".into()));
        second.set_group_id(Some("align".into()));
        first.set_checked(true);
        second.set_checked(true);
        assert!(first.is_checked() && second.is_checked());
    }

    #[test]
    fn auto_raise_is_off_by_default_and_writable() {
        use crate::widget::capability::WidgetProperties;
        let mut button = ToggleButton::new("Bold".to_string(), Rect::new(0, 0, 80, 30));
        assert!(!button.is_auto_raise());
        button.set("auto_raise", CapabilityValue::Bool(true)).expect("writable");
        assert!(button.is_auto_raise());
        assert_eq!(button.get("auto_raise"), Ok(CapabilityValue::Bool(true)));
    }

    #[test]
    fn an_icon_source_is_readable_and_forwarded_to_the_base_hook() {
        use crate::widget::capability::WidgetProperties;
        use crate::widget::IconName;
        let mut button = ToggleButton::new("Bold".to_string(), Rect::new(0, 0, 80, 30));
        assert_eq!(button.get("icon_source"), Ok(CapabilityValue::Null));
        button.set_icon_source(Some(IconSource::Builtin(IconName::Info)));
        let expected = crate::widget::icon_source::IconSpec::Builtin(IconName::Info);
        assert_eq!(button.get("icon_source"), Ok(CapabilityValue::IconSpec(expected)));
        assert!(button.size_hint().width > 0);
    }

    #[cfg(full_widgets)]
    #[test]
    fn an_attached_menu_opens_on_click_and_reports_its_row() {
        use crate::widget::menu_toolbar::menu_button::{menu_item_rect, MenuItem};
        use std::sync::{Arc, Mutex};

        let mut button = ToggleButton::new("Bold".to_string(), Rect::new(0, 0, 80, 30));
        button.set_menu(vec![MenuItem::new(9, "Toggle")]);
        let triggered = Arc::new(Mutex::new(0u64));
        let sink = triggered.clone();
        button.item_triggered.connect(move |id| {
            *sink.lock().unwrap() = *id;
        });

        button.handle_event(&Event::MousePress { pos: Point::new(5, 5), button: 1, modifiers: 0 });
        assert!(button.is_menu_open(), "the instant-popup mode opens on press");
        assert!(!button.is_checked(), "opening the menu is not the face's toggle");

        let row = menu_item_rect(button.menu_rect(), 0);
        button.handle_event(&Event::MousePress {
            pos: Point::new(row.x + 3, row.y + 3),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(*triggered.lock().unwrap(), 9);
        assert!(!button.is_menu_open());
    }

    #[cfg(full_widgets)]
    #[test]
    fn a_menu_button_popup_splits_the_face_from_the_arrow() {
        use crate::widget::menu_toolbar::menu_button::{MenuArrowType, MenuButtonPopup, MenuItem};
        let mut button = ToggleButton::new("Bold".to_string(), Rect::new(0, 0, 80, 30));
        button.set_menu(vec![MenuItem::new(1, "One")]);
        button.set_arrow_type(MenuArrowType::Down);
        button.set_popup_mode(MenuButtonPopup::MenuButtonPopup);

        button.handle_event(&Event::MousePress {
            pos: Point::new(10, 10),
            button: 1,
            modifiers: 0,
        });
        assert!(!button.is_menu_open(), "a face press does not open the menu");
        button.handle_event(&Event::MouseRelease { pos: Point::new(10, 10), button: 1 });
        assert!(button.is_checked(), "the face toggles on release");
        assert!(!button.is_menu_open());

        button.handle_event(&Event::MousePress {
            pos: Point::new(70, 10),
            button: 1,
            modifiers: 0,
        });
        assert!(button.is_menu_open(), "the arrow segment opens the menu");
    }
}

#[cfg(test)]
mod checked_visibility_tests {
    use super::*;
    use crate::event::EventHandler;

    /// A latched toggle reports `Checked`, which is what makes `toggle_button:checked` reachable.
    ///
    /// # The defect this pins
    ///
    /// The trait's default `widget_state` knows only `disabled`/`pressed`/`hovered`/`focused`, and
    /// **none of them means "latched"** — so a checked toggle reported `Normal`, the theme's
    /// `"<kind>:<state>"` lookup could never reach a checked key, and this control had no such key at
    /// all. `CheckBox` was fixed for the same reason; `ToggleButton` was missed.
    #[test]
    fn a_latched_toggle_reports_checked() {
        use crate::style::WidgetState;
        let mut button = ToggleButton::new("On".to_string(), Rect::new(0, 0, 100, 30));
        assert_eq!(button.widget_state(), WidgetState::Normal);
        button.set_checked(true);
        assert_eq!(button.widget_state(), WidgetState::Checked);
        // The latch outranks the momentary states, as it does for `CheckBox`: a checked toggle that is
        // hovered is still checked, and that fill is the fact a user is looking for.
        button
            .handle_event(&crate::event::Event::MouseEnter { pos: crate::core::Point::new(1, 1) });
        assert_eq!(button.widget_state(), WidgetState::Checked);
    }

    /// A checked toggle **paints differently** from an unchecked one.
    ///
    /// # The defect this pins, and why the assertion is on the drawing
    ///
    /// `draw` has always branched on `self.checked`, but it read `style.background_color` first — and
    /// the theme always supplies one for this control — so the branch was **dead code**. The two
    /// states rendered byte-identical, which the snapshot set showed as `toggle_button_checked.svg`
    /// differing from `toggle_button.svg` only in its provenance comment. Asserting
    /// `is_checked() == true` would have passed throughout; only the emitted geometry can catch it.
    #[cfg(device_profile)]
    #[test]
    fn a_checked_toggle_looks_different_from_an_unchecked_one() {
        let bounds = Rect::new(0, 0, 120, 40);

        let mut unchecked = ToggleButton::new("On".to_string(), bounds);
        crate::theme::apply_theme_to_widget(&mut unchecked);
        let plain = crate::widget::svg::render_widget_to_svg(&mut unchecked, bounds);

        let mut checked = ToggleButton::new("On".to_string(), bounds);
        checked.set_checked(true);
        crate::theme::apply_theme_to_widget(&mut checked);
        let latched = crate::widget::svg::render_widget_to_svg(&mut checked, bounds);

        assert_ne!(
            plain, latched,
            "a toggle whose on state draws the same as its off state is not a toggle"
        );
    }

    // ── D09-STYLE-02: the hint measures with the font the paint uses ──

    /// A caller-authored `style.font` must widen the toggle hint.
    #[test]
    fn a_custom_font_grows_the_toggle_button_hint() {
        let baseline = ToggleButton::new("A wide label".to_string(), Rect::new(0, 0, 200, 40));
        let base_hint = baseline.size_hint();

        let mut big = ToggleButton::new("A wide label".to_string(), Rect::new(0, 0, 200, 40));
        big.set_style(
            crate::style::WidgetStyle::default().with_font(crate::core::Font::simple("Test", 30.0)),
        );
        let big_hint = big.size_hint();

        assert!(
            big_hint.width > base_hint.width,
            "a larger font must widen the toggle hint: {} vs {}",
            big_hint.width,
            base_hint.width
        );
    }
}
