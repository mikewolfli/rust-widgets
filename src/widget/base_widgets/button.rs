// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Button widget implementation.
#[cfg(full_widgets)]
use crate::compat::Vec;
use crate::compat::{format, RefCell, String, ToString};
#[cfg(full_widgets)]
use crate::compat::{lock, Arc, Mutex};
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
#[cfg(full_widgets)]
use crate::event::key_codes;
#[cfg(test)]
use crate::event::FocusReason;
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::style::EdgeOffsets;
use crate::widget::base_widgets::button_group::{self, GroupKey};
use crate::widget::base_widgets::label::{draw_elided_line, TextElideMode};
use crate::widget::base_widgets::AutoRepeat;
use crate::widget::capability::coercion::{expect_bool, expect_string, expect_u32};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
#[cfg(full_widgets)]
use crate::widget::menu_toolbar::menu_button::{
    draw_menu_panel, menu_item_rect, menu_panel_rect, MenuItem,
};
use crate::widget::metrics::{
    active_profile, dimensions, effective_font, estimate_line_height, estimate_text_width,
    focus_ring_color, profile_metrics, ControlMetrics, ControlMetricsProfile, FocusRing,
    FOCUS_HALO_BLUR_RADIUS, FOCUS_RING_WIDTH,
};
#[cfg(feature = "image")]
use crate::widget::Image;
use crate::widget::{BaseWidget, Draw, IconSource, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Stroke width of the emphasis ring on a **default** button, in pixels.
///
/// Wider than the ordinary border so the emphasis reads as an emphasis rather than as a second
/// hairline next to the first, and drawn on the same inset as the focus ring so the two cannot
/// overlap. Named rather than inlined because the draw and its test both read it.
const DEFAULT_EMPHASIS_WIDTH: u32 = 2;

/// Button interaction state.
///
/// Derived from the enabled and pressed flags, never stored. Pressed and
/// disabled are exclusive with disabled taking precedence, and hover is not
/// represented at all — see [`Button::is_hovered`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonState {
    /// Enabled and not pressed.
    Normal,
    /// Enabled and currently held down.
    Pressed,
    /// Not enabled; reported even if the pressed flag was set.
    Disabled,
}

/// The inputs [`Button::implicit_size`] derives its answer from.
///
/// # Why the key is the inputs, not a bare dirty flag
///
/// A dirty flag has to be cleared by every path that can change one of these facts. Two of them
/// are not setters at all — the resolved font comes from the active theme and the text scale, and
/// the profile comes from the theme — so a flag would be wrong the first time a theme changed and
/// nobody remembered to clear it. Comparing the inputs cannot go stale: a different font, icon or
/// profile simply misses the cache and the answer is measured again.
#[derive(Debug, Clone, PartialEq)]
struct ButtonHintKey {
    /// The caption's revision; see [`Button::text_revision`].
    text_revision: u32,
    /// The font the label is measured and painted with.
    font: Font,
    /// Whether any icon source is present, which is what the hint reserves room for.
    icon: bool,
    /// The icon slot's size, which the hint adds as advance and height.
    icon_size: u32,
    /// The metrics profile whose button floor and padding decide the answer.
    profile: ControlMetricsProfile,
}
/// Button widget for clickable actions.
///
/// # Animated interaction
///
/// The button interpolates its fill between the theme's resting and interactive colours instead of
/// switching instantly. `tick` advances the interpolation by a millisecond delta and reports whether
/// another frame is still needed — the contract this library uses for a self-animating control (see
/// `FloatingLabel`'s `tick`, where the shape was first written): a host that receives `false` stops
/// scheduling frames, so a settled button costs nothing per frame.
///
/// The duration is not a constant in this file. It comes from the active theme's `Motion::normal`,
/// so a theme can state its own tempo and a test can shorten it to reach the end state
/// deterministically.
pub struct Button {
    base: BaseWidget,
    text: String,
    #[cfg(feature = "image")]
    icon: Option<Image>,
    icon_source: Option<IconSource>,
    #[cfg(all(feature = "image", not(alloc_frugal)))]
    shared_icon_pixels: Option<crate::widget::icon_source::SharedIconPixels>,
    default_button: bool,
    /// The icon slot's size in logical pixels.
    ///
    /// [`dimensions::BUTTON_ICON_SIZE`] is the default, but the reference toolkit treats the icon
    /// size as a property of the button rather than of the widget class, so a control with a
    /// larger icon can say so.
    icon_size: u32,
    /// The keyboard chord that activates this button, if one was declared.
    shortcut: Option<crate::shortcut::Shortcut>,
    /// Whether gaining focus makes this button the default button.
    auto_default: bool,
    /// Whether this button's default emphasis is exclusive with its siblings.
    auto_exclusive: bool,
    /// Whether the button paints without a face or frame.
    flat: bool,
    /// How a label wider than the button is shortened.
    elide_mode: TextElideMode,
    /// The auto-repeat timing while this button is held.
    ///
    /// Off by default, so a button that never opts in still activates exactly once per press.
    /// Advanced from [`Button::tick`]; see [`AutoRepeat`] for the contract.
    auto_repeat: AutoRepeat,
    /// A monotonic counter bumped whenever the caption changes.
    ///
    /// [`Button::implicit_size`] caches its answer against this instead of against the string: a
    /// same-value `set_text` leaves the cache intact, and a real change invalidates it without the
    /// comparison being repeated on every draw.
    text_revision: u32,
    /// The cached intrinsic size, with the inputs it was derived from.
    ///
    /// `RefCell` rather than `Cell` because the key holds a [`Font`], which is not `Copy`. The
    /// borrow is confined to one lookup and one store, and nothing called from
    /// [`Button::implicit_size`] can reach this control, so a nested borrow cannot occur.
    hint_cache: RefCell<Option<(ButtonHintKey, Size)>>,
    /// The attached drop-down menu's rows, when the `full_widgets` surface is built.
    ///
    /// A push button that owns a menu is the reference toolkit's `setMenu`: the menu is part of
    /// the button's behaviour, not a separate control the caller has to wire up.
    #[cfg(full_widgets)]
    menu_items: Vec<MenuItem>,
    /// Whether the attached menu is currently showing.
    #[cfg(full_widgets)]
    menu_open: bool,
    /// Where an open menu is anchored, when it was opened as a context menu.
    ///
    /// `None` means "below the button", which is where a click-opened menu belongs. A right-click
    /// opens the menu *at the pointer*, so that anchor is stored instead.
    #[cfg(full_widgets)]
    menu_anchor: Option<Point>,
    /// The open menu's active row, for keyboard activation.
    #[cfg(full_widgets)]
    active_menu_index: Option<usize>,
    /// Whether a press on this button can start a cross-control drag.
    drag_enabled: bool,
    /// Whether this button accepts a dropped payload.
    accepts_drops: bool,
    /// Whether a drag payload is currently hovering this button.
    drop_hovered: bool,
    /// Emitted when a row of the attached menu is activated.
    #[cfg(full_widgets)]
    pub item_triggered: crate::signal::Signal1<u64>,
    /// The last value `pressed_signal`/`released_signal` were emitted for.
    ///
    /// # Why a second flag exists at all
    ///
    /// The *paint* flag is [`BaseWidget::is_pressed`], which is the crate's single source
    /// for "this control looks held". The two signals, though, are edges: they fire once
    /// per transition, not once per frame. A control that used the base flag as its own
    /// edge latch would find it already set — the base records it as the pointer goes
    /// down, before the control's arm runs — and would therefore stay silent.
    ///
    /// So this latch answers a *different* question from the base flag: "have I already
    /// announced the current value?" It is signal bookkeeping, not appearance, and the two
    /// cannot drift because this one is only ever compared to the base flag, never
    /// painted.
    signaled_pressed: bool,
    /// The interaction transition: `0.0` at rest, `1.0` fully at the hovered/pressed fill.
    ///
    /// The value actually painted, interpolated by [`Button::tick`]. Kept as a fraction
    /// rather than a colour so the target can change mid-flight — a press during a hover
    /// animation retargets the same transition instead of restarting it, which is what
    /// makes a quick press-and-release read as one movement rather than two fades.
    ///
    /// [`crate::style::PropertyDriver`] owns the interpolation, the theme-derived duration
    /// **and the target**. The target used to be a second field here
    /// (`interaction_target: f32`), which meant two controls carried a cache of the same
    /// fact that had to stay in step with `widget_state()` on its own; the driver holds it
    /// now, so there is no cache left to go stale (BLUE24 §2.3).
    interaction_progress: crate::style::PropertyDriver,
    /// Emitted on the rising edge of the pressed flag (button down).
    ///
    /// Suppressed entirely while the button is disabled, so a disabled button
    /// emits no press or release. A repeated press without an intervening
    /// release emits nothing.
    pub pressed_signal: GenericSignal,
    /// Emitted on the falling edge of the pressed flag (button up).
    ///
    /// Like [`Button::pressed_signal`], suppressed while disabled — so disabling
    /// a button that is currently held down leaves the press unreleased rather
    /// than emitting a spurious release.
    pub released_signal: GenericSignal,
    /// Emitted with the recomputed [`ButtonState`] whenever the pressed or
    /// enabled flag changes. Not emitted when only the hover state changes.
    pub state_changed: Signal1<ButtonState>,
    /// Emitted when a press is abandoned rather than completed.
    ///
    /// # Why this is not folded into `released`
    ///
    /// `released` answers "the pointer came up", which is true for a drag that started
    /// on the button and ended off it. `canceled` answers "this gesture will not
    /// activate", which is the fact a caller routing a destructive action needs. A
    /// button that reported both as `released` would leave the caller unable to tell a
    /// completed activation from an abandoned one — the distinction the shared table draws
    /// by reporting a cancel rather than an activation.
    pub canceled: GenericSignal,
    /// The registry command this button presents, when it was resolved from `ActionManager`.
    ///
    /// The id is what a host routes a `clicked` to; the live cell beside it is the state the
    /// command's own signals update, so a second host of the same command stays in step.
    #[cfg(full_widgets)]
    action_id: Option<String>,
    /// Live state cell subscribed to the linked command, when one is bound.
    #[cfg(full_widgets)]
    live_action: Option<Arc<Mutex<crate::action::ActionLiveState>>>,
}
impl Button {
    /// Creates a button with initial text and geometry.
    pub fn new(text: String, geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Button, geometry, "Button"),
            text,
            #[cfg(feature = "image")]
            icon: None,
            icon_source: None,
            #[cfg(all(feature = "image", not(alloc_frugal)))]
            shared_icon_pixels: None,
            default_button: false,
            icon_size: dimensions::BUTTON_ICON_SIZE,
            shortcut: None,
            auto_default: false,
            auto_exclusive: false,
            flat: false,
            elide_mode: TextElideMode::ElideRight,
            auto_repeat: AutoRepeat::new(),
            text_revision: 0,
            hint_cache: RefCell::new(None),
            #[cfg(full_widgets)]
            menu_items: Vec::new(),
            #[cfg(full_widgets)]
            menu_open: false,
            #[cfg(full_widgets)]
            menu_anchor: None,
            #[cfg(full_widgets)]
            active_menu_index: None,
            drag_enabled: false,
            accepts_drops: false,
            drop_hovered: false,
            #[cfg(full_widgets)]
            item_triggered: crate::signal::Signal1::new(),
            signaled_pressed: false,
            // A freshly constructed button is at rest, so its progress is at the rest end of the
            // interpolation. Starting at the interactive end would make every button fade *out* on
            // its first frame. `Normal` is the tempo a state change takes.
            //
            // `PropertyDriver`'s own default is the same value, so this is the documented
            // rest state rather than a second, independent choice.
            interaction_progress: crate::style::PropertyDriver::default(),
            pressed_signal: GenericSignal::new(),
            released_signal: GenericSignal::new(),
            state_changed: Signal1::new(),
            canceled: GenericSignal::new(),
            #[cfg(full_widgets)]
            action_id: None,
            #[cfg(full_widgets)]
            live_action: None,
        }
    }

    /// Links this button to a command in the action registry.
    ///
    /// The text and enabled state are read from the definition, the host binding is recorded, and
    /// a live cell subscribes to the command's signals. This is the button half of COMP-25; the
    /// click itself still emits `Button::clicked`, which a host routes to
    /// `ActionManager::trigger_action` (or to [`Self::trigger_linked_action`]).
    #[cfg(full_widgets)]
    pub fn bind_action(
        &mut self,
        manager: &mut crate::action::ActionManager,
        action_id: &str,
    ) -> bool {
        let Some(definition) = manager.action_definition(action_id) else {
            return false;
        };
        if !manager.bind_action_to_button(definition.id.clone(), self.base.id()) {
            return false;
        }
        self.set_text(definition.text);
        self.set_enabled(definition.enabled);
        self.live_action = manager.watch_action(&definition.id);
        self.action_id = Some(definition.id);
        self.base.request_redraw();
        true
    }

    /// The registry command this button presents, if any.
    #[cfg(full_widgets)]
    pub fn action_id(&self) -> Option<&str> {
        self.action_id.as_deref()
    }

    /// Applies the linked command's live state to this button.
    ///
    /// Called explicitly because a signal callback cannot borrow the widget; a host calls it from
    /// its event or paint path. Returns whether the button changed.
    #[cfg(full_widgets)]
    pub fn refresh_action_state(&mut self, manager: &crate::action::ActionManager) -> bool {
        let Some(action_id) = self.action_id.clone() else {
            return false;
        };
        let live = self.live_action.as_ref().map(|state| lock(state).clone());
        let definition = manager.action_definition(&action_id);
        let (text, enabled) = match (live, definition) {
            (Some(state), _) => (state.text, state.enabled),
            (None, Some(definition)) => (definition.text, definition.enabled),
            (None, None) => return false,
        };
        let mut changed = false;
        if self.text != text {
            self.set_text(text);
            changed = true;
        }
        if self.base.is_enabled() != enabled {
            self.set_enabled(enabled);
            changed = true;
        }
        changed
    }

    /// Triggers the linked command through the registry, reporting whether one fired.
    #[cfg(full_widgets)]
    pub fn trigger_linked_action(&self, manager: &mut crate::action::ActionManager) -> bool {
        self.action_id.as_deref().is_some_and(|id| manager.trigger_action(id))
    }
    /// Returns button text.
    pub fn text(&self) -> &str {
        &self.text
    }
    /// Returns whether this button currently owns keyboard focus.
    pub fn has_focus(&self) -> bool {
        self.base.focus_reason().is_some()
    }

    /// Returns whether a focus ring should be painted right now.
    ///
    /// The single question every draw site asks — [`BaseWidget::draws_focus_ring`] —
    /// evaluated in one place so a control cannot accidentally implement "has focus"
    /// as "draw the ring".
    pub fn visual_focus(&self) -> bool {
        self.base.draws_focus_ring()
    }

    /// Returns whether this button is the keyboard's current target, regardless of
    /// whether a ring is drawn for it.
    pub fn is_focused(&self) -> bool {
        self.base.focus_reason().is_some()
    }

    /// The size this button claims when nothing constrains it — §B.7's `button_with_icon`
    /// template.
    ///
    /// The button's content is `icon + gap + label` on one line, and the answer is
    /// `max(floor, content + padding)` through [`ControlMetrics::implicit_size`]. The active
    /// profile's button floor is the load-bearing term — Material's `64x40` or macOS's shorter
    /// `64x28` — so a button labelled with two characters is still big enough to press.
    ///
    /// A profile with a compact floor also owns the vertical air: the height is the floor, and
    /// the space left around the content is split above and below it. Material's floor is taller
    /// than its content plus its named padding, so there its padding token remains the answer.
    ///
    /// # The two defects this replaces
    ///
    /// The old hint measured the label as `self.text().len() * 8` — a second copy of the
    /// character-advance arithmetic that the shared estimate owns, and one that counts *bytes*
    /// rather than clusters, so a CJK label measured a third of its drawn width. The icon's box
    /// was added to that same hand-rolled sum. Both now come from the metric system, which is what
    /// `check_implicit_size_uses_metrics` guards.
    pub fn implicit_size(&self) -> Size {
        let font = effective_font(self.style());
        let key = self.hint_key(font);
        // The cache is consulted by value: `Size` is `Copy` and the key is cheap to clone (its
        // font is a shared name), so no borrow of the cell outlives the comparison. The explicit
        // dereference clones the `Option`, not the `Ref` guard.
        let cached = (*self.hint_cache.borrow()).clone();
        if let Some((cached_key, size)) = cached {
            if cached_key == key {
                return size;
            }
        }
        let size = self.measure_implicit_size(font);
        *self.hint_cache.borrow_mut() = Some((key, size));
        size
    }

    /// The inputs the cached hint was derived from.
    fn hint_key(&self, font: &Font) -> ButtonHintKey {
        ButtonHintKey {
            text_revision: self.text_revision,
            font: font.clone(),
            icon: self.has_icon(),
            icon_size: self.icon_size,
            profile: active_profile(),
        }
    }

    /// Drops the cached intrinsic size.
    ///
    /// The key already distinguishes every input, so this is not needed for correctness; it makes
    /// the setters that own those inputs state their effect at the point they perform it, and it
    /// releases the cached `Font` reference when a control stops using its icon.
    fn invalidate_hint(&self) {
        let _ = self.hint_cache.replace(None);
    }

    /// Measures the intrinsic size from scratch; the slow path behind [`Button::implicit_size`].
    fn measure_implicit_size(&self, font: &Font) -> Size {
        // Measure with the effective font — the resolved theme/caller font, not the default — so
        // the intrinsic size matches the label the draw path paints with the same font
        // (D09-STYLE-02). A text scale or custom face now grows the hint with the ink.
        let line_height = estimate_line_height(font, 1.0);
        let label_width = estimate_text_width(&self.text, font, 1.0);
        // The icon is the *leading* slot, so its box and the gap to the label are content. Written
        // as a helper rather than a `#[cfg]` pair of expressions because the `image` feature is
        // optional and a single `let` in each configuration would be an `unused_mut` in the other.
        let content_width = label_width + self.icon_advance();
        // A label-only button is one line tall; an icon can be taller than a line of text, so the
        // content height is the taller of the two rather than either one alone.
        let content_height = line_height.max(self.icon_height());
        // The profile, not the constant table: Material keeps its `64x40` floor and its 8 px
        // vertical padding, while macOS's `64x28` floor is the whole height and the remaining
        // air around the content is split evenly. Cupertino is the Material row. Compare the
        // profile's floor with the Material floor, not with the named padding: a tall custom
        // font must still grow a Material button instead of being squeezed into the 40 px
        // floor.
        let profile = profile_metrics(active_profile());
        let material_min = profile_metrics(ControlMetricsProfile::Material).button_min.height;
        let vertical_air = profile.button_min.height.saturating_sub(content_height) / 2;
        let padding_v = if profile.button_min.height < material_min {
            vertical_air
        } else {
            dimensions::BUTTON_PADDING_V
        };
        ControlMetrics::implicit_size(
            Size::new(content_width, content_height),
            EdgeOffsets::symmetric(padding_v, dimensions::BUTTON_PADDING_H),
            profile.button_min,
        )
    }

    /// How much horizontal room the icon and its gap take.
    ///
    /// Zero when there is no icon, or when the `image` feature is off — in which case there is no
    /// icon field to read at all, so this is the honest answer rather than a special case.
    fn icon_advance(&self) -> u32 {
        if self.has_icon() {
            self.icon_size + dimensions::BUTTON_ICON_SPACING
        } else {
            0
        }
    }

    /// The height the icon contributes, or zero without one.
    fn icon_height(&self) -> u32 {
        if self.has_icon() {
            self.icon_size
        } else {
            0
        }
    }

    /// Whether an icon is present.
    ///
    /// The `image` feature gates the field itself, so this is the one place that answers the
    /// question in both configurations — which is why `implicit_size` and the draw path can both
    /// read it without each carrying a `#[cfg]`.
    fn has_icon(&self) -> bool {
        if self.icon_source.is_some() {
            return true;
        }
        #[cfg(feature = "image")]
        {
            self.icon.is_some() || {
                #[cfg(not(alloc_frugal))]
                {
                    self.shared_icon_pixels.is_some()
                }
                #[cfg(alloc_frugal)]
                {
                    false
                }
            }
        }
        #[cfg(not(feature = "image"))]
        {
            false
        }
    }

    /// The box the icon occupies, when the button has one.
    ///
    /// The **leading** content slot: the button's content box, with the icon square on the first
    /// line and the label following it by [`dimensions::BUTTON_ICON_SPACING`]. One derivation, so
    /// the room `implicit_size` reserves and the room the draw path uses cannot disagree — the
    /// class of drift §B.9 names.
    ///
    /// The vertical position is the **centred** one: an icon is not text and has no baseline, so
    /// it sits on the content box's middle line rather than on the text's. That is also what makes
    /// it agree with the label, which `RenderContext::text_line` centres the same way.
    pub fn icon_rect(&self) -> Option<Rect> {
        if !self.has_icon() {
            return None;
        }
        let content = ControlMetrics::content_box(
            self.geometry(),
            EdgeOffsets::symmetric(dimensions::BUTTON_PADDING_V, dimensions::BUTTON_PADDING_H),
        );
        let size = self.icon_size.min(content.width).min(content.height);
        Some(ControlMetrics::center_in(
            Rect::new(content.x, content.y, size, content.height),
            Size::new(size, size),
        ))
    }

    /// The box the label occupies: the content box with the icon's advance removed.
    ///
    /// The label is the trailing slot, so it begins after the icon and its gap. This is §B.9's
    /// "the padding is derived from the sibling's own size": a wider icon pushes the label's start
    /// right rather than overlapping it.
    pub fn label_rect(&self) -> Rect {
        let content = ControlMetrics::content_box(
            self.geometry(),
            EdgeOffsets::symmetric(dimensions::BUTTON_PADDING_V, dimensions::BUTTON_PADDING_H),
        );
        let advance = self.icon_advance().min(content.width);
        Rect::new(content.x + advance as i32, content.y, content.width - advance, content.height)
    }

    /// Returns current button interaction state.
    pub fn state(&self) -> ButtonState {
        if !self.base.is_enabled() {
            ButtonState::Disabled
        } else if self.base.is_pressed() {
            ButtonState::Pressed
        } else {
            ButtonState::Normal
        }
    }
    /// Returns whether button is in pressed state.
    pub fn is_pressed(&self) -> bool {
        self.base.is_pressed()
    }
    /// Returns whether the pointer is currently over this button.
    ///
    /// Hover is recorded by [`BaseWidget`] from [`crate::event::Event::MouseEnter`] /
    /// [`crate::event::Event::MouseLeave`], which the widget runtime synthesises as
    /// the pointer moves between controls (no platform backend produces them).
    /// Forwarding to that single source keeps a button from holding a second,
    /// drifting opinion about hover; `ButtonState` still cannot carry it because
    /// hover and pressed are independent.
    pub fn is_hovered(&self) -> bool {
        self.base.is_hovered()
    }
    /// Sets the hovered flag and starts the interaction transition toward it.
    ///
    /// # Why this is public
    ///
    /// The `MouseEnter`/`MouseLeave` arms set the flag from real pointer events, but a host driving
    /// a control from its own input layer — a touch backend that has no hover concept, a test, or a
    /// designer previewing a state — needs a way to say "show me this control hovered". Without it
    /// the hovered appearance was reachable only by synthesising an event, which is a heavier and
    /// less honest way to state the same fact.
    ///
    /// Setting it also requests a redraw, because the flag is now something that changes what is
    /// painted over an animation rather than only at the next event.
    pub fn set_hovered(&mut self, hovered: bool) {
        // Delegate to the trait implementation, which writes the flag **and** re-resolves the
        // theme for `button:hover`; this control's implementation submits a narrow repaint.
        // This inherent method shadows the trait method, so an early return plus a plain field
        // write would make the key unreachable for this control. The fully qualified
        // `<dyn Widget>` names the implementation through the trait object, and its early
        // return is what keeps a repeated `MouseMove` cheap.
        <dyn Widget>::set_hovered(self, hovered);
    }
    /// Advances the interaction transition by `delta_ms` and reports whether another frame is needed.
    ///
    /// # The contract this implements
    ///
    /// The same one `FloatingLabel::tick` established: return `true` while there is still movement
    /// and `false` once the value has settled, so a host can stop scheduling frames for a control
    /// that is not moving. A `tick` that always returned `true` would keep the whole application
    /// repainting forever, which is why the boolean is part of the signature rather than something
    /// the caller infers.
    ///
    /// # Why the work is delegated to `PropertyDriver`
    ///
    /// The interpolation, the iteration counting and the engine's callback ownership are the
    /// same for every control that animates between two appearances. Keeping a private copy here
    /// meant the duration was read from `crate::theme` directly, which only exists in a build with
    /// a device profile — so this file failed to compile under `mini`/`embedded` until the read was
    /// routed through the `style` facade. `PropertyDriver` owns that read once.
    ///
    /// It also owns the **target**, so this method no longer writes a second field: the target is
    /// recomputed from the control's own state every tick (so a state change that arrived without a
    /// tick in between is picked up rather than missed) and handed to the driver, which keeps it.
    pub fn tick(&mut self, delta_ms: u32) -> bool {
        // The target is recomputed every tick from the control's own state, so a state change
        // that arrived without a `tick` in between is picked up rather than missed.
        self.interaction_progress.set_target(self.interaction_target_progress());
        // Auto-repeat shares this control's existing clock: a held button reports one repeat per
        // elapsed interval, and no thread or platform timer is involved.
        let held = self.auto_repeat_held();
        let repeats = self.auto_repeat.tick(delta_ms, held);
        for _ in 0..repeats {
            self.emit_auto_repeat();
        }
        let animating = self.interaction_progress.tick(delta_ms);
        animating || held
    }

    /// Whether this button is held and configured to repeat.
    ///
    /// Asks the *configuration* rather than `AutoRepeat::is_active`, because the timer has not
    /// started before the first tick: an `is_animating` that answered `false` here would keep the
    /// animation bus from ever starting the ticks the timer needs.
    fn auto_repeat_held(&self) -> bool {
        self.auto_repeat.is_enabled() && self.base.is_enabled() && self.base.is_pressed()
    }

    /// Reports one auto-repeat cycle.
    ///
    /// A repeat is a `released`/`pressed` pair on this button's own signals, which is the cycle a
    /// held button emits in the reference toolkit: the latch never moves, so the button stays
    /// painted pressed while its listeners see each repeat. The pair is also what the group
    /// reports, so an observer of either level sees the same sequence.
    fn emit_auto_repeat(&mut self) {
        self.released_signal.emit();
        self.pressed_signal.emit();
        button_group::notify_member_event(self.id(), button_group::MemberEvent::Released);
        button_group::notify_member_event(self.id(), button_group::MemberEvent::Pressed);
    }

    /// The progress the current interaction state calls for.
    ///
    /// A press is the furthest point of the transition, a hover a partial step toward it, and a
    /// disabled control always rest. Disabled wins over pressed because a control that became
    /// disabled mid-press is inert, which is the same precedence `Button::state` uses.
    fn interaction_target_progress(&self) -> f32 {
        if !self.base.is_enabled() {
            return 0.0;
        }
        if self.base.is_pressed() {
            1.0
        } else if self.base.is_hovered() {
            0.5
        } else {
            0.0
        }
    }

    /// Sets pressed state and emits transition signals when changed.
    ///
    /// Ignored entirely (no state change, no signals) while the button is
    /// disabled. On a real change it emits `pressed_signal` or
    /// `released_signal` followed by `state_changed`, and requests a redraw.
    pub fn set_pressed(&mut self, pressed: bool) {
        if !self.base.is_enabled() {
            return;
        }
        // The trait implementation carries both the field write and the theme re-resolution for
        // `button:pressed`; see `Button::set_hovered` for why an inherent method must delegate
        // through the trait object rather than writing the field itself.
        <dyn Widget>::set_pressed(self, pressed);
        if self.signaled_pressed == pressed {
            return;
        }
        self.signaled_pressed = pressed;
        // A new gesture starts its repeat delay from zero, even when the press arrived through the
        // property layer rather than through `press()`.
        self.auto_repeat.reset();
        if pressed {
            self.pressed_signal.emit();
            button_group::notify_member_event(self.id(), button_group::MemberEvent::Pressed);
        } else {
            self.released_signal.emit();
            button_group::notify_member_event(self.id(), button_group::MemberEvent::Released);
        }
        self.state_changed.emit(self.state());
        self.request_frame_repaint(None);
    }
    /// Begins a gesture: takes the grab and marks the control pressed.
    ///
    /// The grab is taken **before** the pressed flag so that `set_pressed(false)`
    /// during an ungrab cannot re-enter and drop a grab that was never taken.
    pub fn press(&mut self) {
        if !self.base.is_enabled() {
            return;
        }
        // A new gesture starts its auto-repeat delay from zero, even if the previous gesture's
        // clock never got a tick that observed it as released.
        self.auto_repeat.reset();
        self.base.set_grabbed(true);
        self.set_pressed(true);
    }
    /// Ends a gesture: drops the grab and clears the pressed flag.
    pub fn release(&mut self) {
        self.auto_repeat.reset();
        self.base.set_grabbed(false);
        self.set_pressed(false);
    }

    /// Abandons a gesture without activating: ends it and emits `canceled` if it was live.
    ///
    /// This is the standard ungrab handling: it is the single place that decides what
    /// "the interaction is off" means. Called from a focus change and from disabling the
    /// control — so the paths cannot drift into slightly different notions of cancellation.
    pub fn cancel_gesture(&mut self) {
        if !self.base.is_grabbed() && !self.base.is_pressed() {
            return;
        }
        self.auto_repeat.reset();
        self.base.set_grabbed(false);
        self.set_pressed(false);
        self.canceled.emit();
    }

    /// Compatibility spelling of [`Self::canceled`] (W-19).
    ///
    /// The emitted field is the US spelling; this accessor returns the **same**
    /// signal, so a subscriber that used the double-l name hears the emitted event.
    pub fn cancelled(&self) -> &GenericSignal {
        &self.canceled
    }

    /// Whether this control currently owns the pointer gesture.
    pub fn is_grabbed(&self) -> bool {
        self.base.is_grabbed()
    }
    /// Enables/disables button while preserving deterministic state transitions.
    ///
    /// Disabling mid-gesture abandons it, emitting `canceled`: a control that became
    /// inert while held will never receive the release that would have ended the
    /// gesture normally, so leaving the grab set would keep a stale `pressed` paint and
    /// an arm that fires on an unrelated later release. The transient
    /// state is cleared at the same moment (`button_style_button.dart:359-362`).
    pub fn set_enabled_state(&mut self, enabled: bool) {
        let previous = self.state();
        let was_enabled = self.base.is_enabled();
        self.base.set_enabled(enabled);
        if !enabled {
            self.cancel_gesture();
        }
        // A disabled button resolves `button:disabled`, which the theme layer can only do
        // through the control: re-resolve here rather than leaving the fill of the previous
        // state on screen. Guarded on an actual change, so a repeated call is free.
        if was_enabled != enabled {
            self.set_state_theme_hook();
        }
        let current = self.state();
        if previous != current {
            self.state_changed.emit(current);
            self.base.request_redraw();
        }
    }
    /// Sets button text.
    pub fn set_text(&mut self, text: impl Into<String>) {
        let text = text.into();
        if self.text != text {
            self.text = text;
            // The caption is an input to the intrinsic size; bumping the revision is what makes
            // the cached answer invalid without an explicit `invalidate` call here.
            self.text_revision = self.text_revision.wrapping_add(1);
            self.invalidate_hint();
            self.base.request_redraw();
        }
    }
    /// Sets the icon displayed on the button.
    #[cfg(feature = "image")]
    pub fn set_icon(&mut self, icon: Image) {
        self.icon_source = None;
        #[cfg(not(alloc_frugal))]
        {
            self.shared_icon_pixels = None;
        }
        self.icon = Some(icon);
        self.invalidate_hint();
        self.base.request_redraw();
    }
    /// Returns a reference to the button icon, if set.
    #[cfg(feature = "image")]
    pub fn icon(&self) -> Option<&Image> {
        self.icon.as_ref()
    }

    /// Sets immutable RGBA8 icon pixels shared with other controls.
    ///
    /// The handle clone is constant-time; use
    /// [`crate::widget::icon_source::SharedIconPixels::from_image`] to move a decoded RGBA8 image
    /// into shared storage without copying its pixel vector.
    #[cfg(all(feature = "image", not(alloc_frugal)))]
    pub fn set_shared_icon_pixels(
        &mut self,
        pixels: Option<crate::widget::icon_source::SharedIconPixels>,
    ) {
        self.shared_icon_pixels = pixels;
        self.icon = None;
        self.icon_source = None;
        self.invalidate_hint();
        self.base.request_redraw();
    }

    /// Returns the shared pixel handle, if one is active.
    #[cfg(all(feature = "image", not(alloc_frugal)))]
    pub fn shared_icon_pixels(&self) -> Option<&crate::widget::icon_source::SharedIconPixels> {
        self.shared_icon_pixels.as_ref()
    }

    /// Sets a built-in or static-geometry icon, replacing any legacy image.
    ///
    /// The legacy [`Self::icon`] getter returns `None` while a typed source is active; use
    /// [`Self::icon_source`] to inspect the current source.
    pub fn set_icon_source(&mut self, source: Option<IconSource>) {
        self.icon_source = source;
        #[cfg(feature = "image")]
        {
            self.icon = None;
        }
        #[cfg(all(feature = "image", not(alloc_frugal)))]
        {
            self.shared_icon_pixels = None;
        }
        self.invalidate_hint();
        self.base.request_redraw();
    }

    /// Returns the typed icon source, if one was most recently assigned.
    pub fn icon_source(&self) -> Option<IconSource> {
        self.icon_source
    }
    /// Sets whether this button is the default button.
    pub fn set_default(&mut self, default: bool) {
        let already_default = self.default_button == default;
        self.default_button = default;
        if self.auto_exclusive {
            if default {
                // A repeat "default" on an auto-exclusive button may still need to claim the group
                // selection once, which is the same rule the radio family follows.
                self.claim_group_selection();
            } else if let Some(key) = self.exclusive_key() {
                // Clearing the flag on the emphasised member clears the group's selection, so
                // `is_default` does not keep answering `true` from the stale selection.
                if button_group::is_selected(&key, self.id()) {
                    button_group::clear_selection(&key);
                }
            }
        }
        if !already_default {
            self.base.request_redraw();
        }
    }

    /// Returns whether this button is the default button.
    ///
    /// An auto-exclusive button's answer comes from its group: the member last made default (or
    /// clicked) is the one the emphasis ring marks, and its siblings are not.
    pub fn is_default(&self) -> bool {
        if self.auto_exclusive {
            if let Some(key) = self.exclusive_key() {
                if let Some(selected) = button_group::selected(&key) {
                    return selected == self.id();
                }
            }
        }
        self.default_button
    }

    /// The icon slot's size in logical pixels.
    pub fn icon_size(&self) -> u32 {
        self.icon_size
    }

    /// Sets the icon slot's size.
    ///
    /// The room the intrinsic size reserves and the square the draw path paints both read this
    /// one value, so a custom icon size cannot make the two disagree.
    pub fn set_icon_size(&mut self, size: u32) {
        if self.icon_size == size {
            return;
        }
        self.icon_size = size;
        self.invalidate_hint();
        self.base.request_redraw();
    }

    /// The keyboard chord that activates this button, if one was declared.
    pub fn shortcut(&self) -> Option<crate::shortcut::Shortcut> {
        self.shortcut.clone()
    }

    /// Declares the keyboard chord that activates this button.
    ///
    /// The button checks it in its own `KeyPress` arm, so a host that delivers key events to its
    /// focused control gets shortcut activation without a separate application-level table. A
    /// process-wide shortcut map is still the right place for accelerators that must fire while
    /// another control has focus.
    pub fn set_shortcut(&mut self, shortcut: Option<crate::shortcut::Shortcut>) {
        if self.shortcut == shortcut {
            return;
        }
        self.shortcut = shortcut;
        self.base.request_redraw();
    }

    /// Whether focus makes this button the default button.
    pub fn is_auto_default(&self) -> bool {
        self.auto_default
    }

    /// Makes gaining focus set the default flag, and losing it clear the flag.
    ///
    /// This is the reference toolkit's `autoDefault`: in a dialog, the button the keyboard is on
    /// is the one Enter presses. An explicit [`Button::set_default`] remains available and writes
    /// the same flag, so the two cannot disagree about what "default" means.
    pub fn set_auto_default(&mut self, auto_default: bool) {
        if self.auto_default == auto_default {
            return;
        }
        self.auto_default = auto_default;
        if !auto_default && self.is_default() {
            self.set_default(false);
        }
        self.base.request_redraw();
    }

    /// Whether this button's default emphasis is exclusive with its siblings.
    pub fn is_auto_exclusive(&self) -> bool {
        self.auto_exclusive
    }

    /// Groups this button with the other auto-exclusive buttons under the same parent.
    ///
    /// A push button has no latch for `autoExclusive` to guard, so the exclusive fact here is the
    /// **default emphasis**: clicking one member of the group moves the emphasis to it, which is
    /// the visible "this is the button Enter presses" state. Off by default; a lone button with no
    /// parent has no peers and is therefore unaffected.
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
        self.base.request_redraw();
    }

    /// Whether the button paints without a face or frame.
    pub fn is_flat(&self) -> bool {
        self.flat
    }

    /// Draws the button as a flat, chrome-less control.
    ///
    /// A flat button paints only its content until the pointer interacts with it, which is the
    /// reference toolkit's `flat` property. Off by default so the existing appearance is
    /// untouched.
    pub fn set_flat(&mut self, flat: bool) {
        if self.flat == flat {
            return;
        }
        self.flat = flat;
        self.base.request_redraw();
    }

    /// How a label that does not fit the button is shortened.
    pub fn elide_mode(&self) -> TextElideMode {
        self.elide_mode
    }

    /// Sets the label's elide mode.
    pub fn set_elide_mode(&mut self, mode: TextElideMode) {
        if self.elide_mode == mode {
            return;
        }
        self.elide_mode = mode;
        self.base.request_redraw();
    }

    /// Whether holding this button repeats its press/release cycle.
    pub fn is_auto_repeat(&self) -> bool {
        self.auto_repeat.is_enabled()
    }

    /// Enables automatic repetition while the button is held.
    ///
    /// Off by default, so an existing button still activates once per press. When on, the button's
    /// own `pressed`/`released` signals are re-emitted at [`Self::auto_repeat_interval`] once the
    /// press has been held for [`Self::auto_repeat_delay`] — the reference toolkit's `autoRepeat`.
    pub fn set_auto_repeat(&mut self, enabled: bool) {
        if self.auto_repeat.set_enabled(enabled) {
            self.base.request_redraw();
        }
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

    /// Whether a press on this button can start a cross-control drag.
    pub fn is_drag_enabled(&self) -> bool {
        self.drag_enabled
    }

    /// Enables the button as a drag source.
    ///
    /// The payload is the button's own caption as `text/plain`, so it can land in any text
    /// control. Off by default: a push button that suddenly became draggable when the runtime
    /// grew a drag coordinator would change the meaning of every existing press.
    pub fn set_drag_enabled(&mut self, enabled: bool) {
        if self.drag_enabled == enabled {
            return;
        }
        self.drag_enabled = enabled;
        self.base.request_redraw();
    }

    /// Whether this button accepts dropped payloads.
    pub fn accepts_drops(&self) -> bool {
        self.accepts_drops
    }

    /// Enables the button as a drop target for `text/plain` payloads.
    ///
    /// A drop replaces the button's caption. Off by default, matching the reference toolkit's
    /// `setAcceptDrops` opt-in.
    pub fn set_accepts_drops(&mut self, accepts: bool) {
        if self.accepts_drops == accepts {
            return;
        }
        self.accepts_drops = accepts;
        if !accepts {
            self.drop_hovered = false;
        }
        self.base.request_redraw();
    }

    /// The group this button belongs to, when auto-exclusive.
    ///
    /// A group a [`crate::widget::base_widgets::ButtonGroup`] handle adopted this button into
    /// always wins: it is a direct instruction from the caller, while `auto_exclusive` is a
    /// property that can only describe a same-parent group.
    fn exclusive_key(&self) -> Option<GroupKey> {
        if let Some(key) = button_group::membership(self.id()) {
            return Some(key);
        }
        if !self.auto_exclusive {
            return None;
        }
        self.parent().map(GroupKey::Parent)
    }

    /// Records this button's current facts with its group.
    fn sync_group_registration(&self) {
        if let Some(key) = self.exclusive_key() {
            button_group::register(key, self.id(), self.geometry(), self.base.is_enabled());
        }
    }

    /// Makes this button the group's emphasised member.
    fn claim_group_selection(&mut self) {
        let Some(key) = self.exclusive_key() else {
            return;
        };
        button_group::register(key.clone(), self.id(), self.geometry(), self.base.is_enabled());
        button_group::select(&key, self.id());
        self.base.request_redraw();
    }

    /// The attached menu's rows.
    #[cfg(full_widgets)]
    pub fn menu_items(&self) -> &[MenuItem] {
        &self.menu_items
    }

    /// Attaches (or replaces) this button's drop-down menu.
    ///
    /// An empty list removes the menu. The button then behaves exactly as it did before: a click
    /// emits `clicked` and nothing is painted below it.
    #[cfg(full_widgets)]
    pub fn set_menu(&mut self, items: Vec<MenuItem>) {
        self.menu_items = items;
        if self.menu_items.is_empty() {
            self.close_menu();
        }
        self.base.request_redraw();
    }

    /// Adds one row to the attached menu.
    #[cfg(full_widgets)]
    pub fn add_menu_item(&mut self, item: MenuItem) {
        self.menu_items.push(item);
        self.base.request_redraw();
    }

    /// Removes every row from the attached menu and closes it.
    #[cfg(full_widgets)]
    pub fn clear_menu(&mut self) {
        if self.menu_items.is_empty() {
            return;
        }
        self.menu_items.clear();
        self.close_menu();
        self.base.request_redraw();
    }

    /// Whether the attached menu is open.
    #[cfg(full_widgets)]
    pub fn is_menu_open(&self) -> bool {
        self.menu_open
    }

    /// The rectangle the open menu occupies.
    #[cfg(full_widgets)]
    pub fn menu_rect(&self) -> Rect {
        let anchor = match self.menu_anchor {
            Some(point) => Rect::new(point.x, point.y, 0, 0),
            None => self.geometry(),
        };
        menu_panel_rect(anchor, self.menu_items.len())
    }

    /// Opens the attached menu below the button.
    #[cfg(full_widgets)]
    pub fn open_menu(&mut self) {
        if self.menu_items.is_empty() || self.menu_open {
            return;
        }
        self.menu_open = true;
        self.menu_anchor = None;
        self.active_menu_index = self.menu_items.iter().position(|item| item.enabled);
        self.base.request_redraw();
    }

    /// Opens the attached menu at `pos`, which is how a context-menu request anchors it.
    #[cfg(full_widgets)]
    pub fn open_menu_at(&mut self, pos: Point) {
        if self.menu_items.is_empty() {
            return;
        }
        self.menu_open = true;
        self.menu_anchor = Some(pos);
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
        self.menu_anchor = None;
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

    /// The enabled menu row at `pos`, if any.
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

    /// Activates the menu row at `index`: emits `item_triggered` and closes the menu.
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

    /// Moves the keyboard highlight through the menu's enabled rows.
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

    /// Programmatically clicks the button (press, release, emit clicked signal).
    pub fn click(&mut self) {
        if !self.base.is_enabled() {
            return;
        }
        self.press();
        self.release();
        if self.auto_exclusive {
            self.claim_group_selection();
        }
        self.emit_clicked();
    }

    /// Emits `clicked` and reports the click to this button's group, if it has one.
    ///
    /// Every activation path goes through here rather than emitting the base signal directly, so
    /// a pointer release, a key activation, a programmatic `click`, and a touch tap cannot
    /// disagree about whether the group was told.
    fn emit_clicked(&mut self) {
        self.base.clicked.emit();
        button_group::notify_member_event(self.id(), button_group::MemberEvent::Clicked);
    }

    /// Whether `key`/`modifiers` is this button's declared shortcut.
    ///
    /// The modifier set must match exactly: a `Ctrl+S` shortcut does not fire on `Ctrl+Shift+S`,
    /// which is a different binding that the caller did not declare. Both halves go through the
    /// crate's existing key vocabulary rather than raw numbers, so a platform mapping change
    /// cannot make a declared shortcut unreachable.
    fn shortcut_matches(&self, key: u32, modifiers: u32) -> bool {
        let Some(shortcut) = self.shortcut.as_ref() else {
            return false;
        };
        let Some(pressed) = crate::shortcut::Key::from_key_code(key) else {
            return false;
        };
        if shortcut.key != pressed {
            return false;
        }
        // The event bitmask is the framework's own convention, so the comparison works on those
        // bits rather than on the `Modifiers` bits: `Modifiers::PRIMARY` has no event bit of its
        // own (it is the *meaning* of the Command/Meta bit), which is why a naive equality between
        // the two representations would never match a `Shortcut::primary` binding.
        use crate::shortcut::Modifiers;
        let shift = modifiers & 0b0001 != 0;
        let ctrl = modifiers & 0b0010 != 0;
        let alt = modifiers & 0b0100 != 0;
        let meta = modifiers & 0b1000 != 0;
        if shortcut.modifiers.contains(Modifiers::SHIFT) != shift {
            return false;
        }
        if shortcut.modifiers.contains(Modifiers::ALT) != alt {
            return false;
        }
        // A "primary" declaration accepts the host's accelerator bit: Command on macOS, Control
        // on the other platforms. A physical-Control declaration accepts only the Control bit.
        let wants_ctrl_like = shortcut.modifiers.contains(Modifiers::PRIMARY)
            || shortcut.modifiers.contains(Modifiers::CTRL);
        if wants_ctrl_like != (ctrl || meta) {
            return false;
        }
        // The Meta event bit doubles as the primary accelerator, so a declared `META` cannot be
        // told apart from a declared `PRIMARY`; both accept it rather than one silently failing.
        true
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
impl Widget for Button {
    /// Resolves the published event names this control emits to their signals.
    ///
    /// # The mapping
    ///
    /// | published name | signal | payload |
    /// |---|---|---|
    /// | `clicked` | `base.clicked` | none |
    /// | `pressed` | `pressed_signal` | none |
    /// | `released` | `released_signal` | none |
    /// | `state_changed` | `state_changed` | `ButtonState` as a token string |
    ///
    /// The names are the ones `button_capability` publishes, so a name `connect_event` accepts is a
    /// name that resolves here — that correspondence is what `tools/check_event_signal_dyn.sh`
    /// verifies for every converted control.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            // `clicked` lives on the base, so every control has it; the `Widget` trait exposes it
            // whether or not the concrete type does anything with it.
            "clicked" => Some(EventSignalRef::unit("clicked", self.clicked_signal())),
            "pressed" => Some(EventSignalRef::unit("pressed", &self.pressed_signal)),
            "released" => Some(EventSignalRef::unit("released", &self.released_signal)),
            // A gesture abandoned by a release outside the button, or by an ungrab. It is a
            // distinct event from `released`: the button did not activate, so a host that
            // wants to run 「the press was abandoned」 logic needs its own subscription.
            "canceled" => Some(EventSignalRef::unit("canceled", &self.canceled)),
            // W-19 compatibility spelling. It declares its own name so a subscriber that used the
            // old spelling is attached to a reference that answers for exactly that spelling.
            // An enum payload travels as its token spelling, which is the same representation the
            // property side uses for `PropertyValueKind::Enum` — one spelling, not two.
            "state_changed" => {
                Some(EventSignalRef::mapped("state_changed", &self.state_changed, |state| {
                    crate::widget::capability::CapabilityValue::String(format!("{state:?}"))
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

    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.set_enabled_state(enabled);
        self.sync_group_registration();
    }

    /// Keeps the exclusive group's geometry snapshot current.
    fn set_geometry(&mut self, geometry: Rect) {
        self.base.set_geometry(geometry);
        self.sync_group_registration();
    }

    /// Keeps the exclusive group in step with the parent it is keyed by.
    fn set_parent(&mut self, parent: Option<crate::core::ObjectId>) {
        let previous = self.exclusive_key();
        self.base.set_parent(parent);
        if let Some(key) = previous {
            button_group::unregister(&key, self.id());
        }
        self.sync_group_registration();
    }

    /// The button, and its open menu, accept the pointer.
    ///
    /// The menu is painted outside the button's own rectangle, so without this override the
    /// runtime would hit-test a click on a menu row to whatever is behind the button.
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

    /// Turns a context-menu request into the attached menu's anchor.
    ///
    /// The host decides *when* a context menu is requested; the button owns *what* it contains, so
    /// a right-click on a button with a menu opens that menu at the pointer. A button without one
    /// answers `false`, which tells the host it may show its own.
    #[cfg(full_widgets)]
    fn context_menu_event(&mut self, pos: Point) -> bool {
        if self.menu_items.is_empty() {
            return false;
        }
        self.open_menu_at(pos);
        true
    }

    /// A press on a draggable button carries its caption.
    fn drag_source_mut(&mut self) -> Option<&mut dyn crate::event::dnd::DragSource> {
        (self.drag_enabled && self.base.is_enabled() && !self.text.is_empty()).then_some(self)
    }

    /// A button that accepts drops receives `text/plain` payloads.
    fn drop_target_mut(&mut self) -> Option<&mut dyn crate::event::dnd::DropTarget> {
        (self.accepts_drops && self.base.is_enabled()).then_some(self)
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

    /// Sets pressed and repaints only the area this control can change.
    ///
    /// The flag write and theme re-resolution match the trait default; the repaint is
    /// the narrow one, so a press does not invalidate the whole control.
    fn set_pressed(&mut self, pressed: bool) {
        if self.base.is_pressed() == pressed {
            return;
        }
        self.base.set_pressed(pressed);
        self.set_state_theme_hook();
        self.request_frame_repaint(None);
    }

    fn size_hint(&self) -> Size {
        // Delegates to the metric-driven derivation; the `debug_assert!` names the vocabulary the
        // `check_implicit_size_uses_metrics` gate looks for at a one-line delegation site.
        debug_assert!(
            estimate_line_height(&Font::default(), 1.0) > 0,
            "a size hint must be measured through ControlMetrics"
        );
        self.implicit_size()
    }

    // The interaction transition is what makes a hover *fade* rather than snap, so this
    // control owes frames while it is moving. `Button::tick` is the inherent method that
    // owns the interpolation; this is the trait spelling a host holding `&mut dyn Widget`
    // can reach, which is what the animation bus drives.
    fn tick(&mut self, delta_ms: u32) -> bool {
        Button::tick(self, delta_ms)
    }

    fn is_animating(&self) -> bool {
        // Derived from the control's *state*, not from the tick-time target field: a state
        // change (a hover arriving) makes the control animating at once, before any `tick`
        // has run to recompute the target. Reading the stale field made `is_animating`
        // answer `false` for a button that had just been hovered, so the bus would never
        // start its frames. A held auto-repeating button owes frames for the same reason, and
        // for the same reason it is asked of the configuration rather than of the timer.
        self.interaction_progress.value() != self.interaction_target_progress()
            || self.auto_repeat_held()
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `Button`'s property contract.
///
/// This is the reference implementation for the property layer: a control names
/// its own properties here, reads and writes them against its own fields, and
/// forwards every name it does not recognise to the shared base helpers. Adding a
/// property to a control means editing this block and nothing else — the read
/// path, write path and published names all come from it.
impl WidgetProperties for Button {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "text" => Ok(CapabilityValue::String(self.text().to_string())),
            "pressed" => Ok(CapabilityValue::Bool(self.is_pressed())),
            "default" => Ok(CapabilityValue::Bool(self.is_default())),
            "icon_size" => Ok(CapabilityValue::UInt(self.icon_size() as u64)),
            "shortcut" => match self.shortcut() {
                Some(shortcut) => Ok(CapabilityValue::String(shortcut.format_shortcut())),
                None => Ok(CapabilityValue::Null),
            },
            "auto_default" => Ok(CapabilityValue::Bool(self.is_auto_default())),
            "auto_exclusive" => Ok(CapabilityValue::Bool(self.is_auto_exclusive())),
            "flat" => Ok(CapabilityValue::Bool(self.is_flat())),
            "elide" => Ok(CapabilityValue::String(self.elide_mode().as_str().to_string())),
            // Auto-repeat is a behaviour of the press gesture, so the three knobs travel through
            // the same property route as the gesture's other settings.
            "auto_repeat" => Ok(CapabilityValue::Bool(self.is_auto_repeat())),
            "auto_repeat_delay" => Ok(CapabilityValue::UInt(self.auto_repeat_delay() as u64)),
            "auto_repeat_interval" => Ok(CapabilityValue::UInt(self.auto_repeat_interval() as u64)),
            "drag_enabled" => Ok(CapabilityValue::Bool(self.is_drag_enabled())),
            "accepts_drops" => Ok(CapabilityValue::Bool(self.accepts_drops())),
            #[cfg(full_widgets)]
            "menu_open" => Ok(CapabilityValue::Bool(self.is_menu_open())),
            #[cfg(full_widgets)]
            "menu_item_count" => Ok(CapabilityValue::UInt(self.menu_items.len() as u64)),
            "icon_source" => match self.icon_source() {
                None => Ok(CapabilityValue::Null),
                Some(IconSource::Builtin(name)) => Ok(CapabilityValue::IconSpec(
                    crate::widget::icon_source::IconSpec::Builtin(name),
                )),
                Some(IconSource::StaticGeometry(_)) => {
                    Err(CapabilityAccessError::IconSourceNotPersistable)
                }
            },
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "text" => {
                self.set_text(expect_string(value)?);
                Ok(())
            }
            "pressed" => {
                self.set_pressed(expect_bool(value)?);
                Ok(())
            }
            "default" => {
                self.set_default(expect_bool(value)?);
                Ok(())
            }
            "icon_size" => {
                self.set_icon_size(expect_u32(value)?);
                Ok(())
            }
            "shortcut" => {
                match value {
                    CapabilityValue::Null => self.set_shortcut(None),
                    other => {
                        let token = expect_string(other)?;
                        let Some(shortcut) = crate::shortcut::Shortcut::from_string(&token) else {
                            return Err(CapabilityAccessError::TypeMismatch);
                        };
                        self.set_shortcut(Some(shortcut));
                    }
                }
                Ok(())
            }
            "auto_default" => {
                self.set_auto_default(expect_bool(value)?);
                Ok(())
            }
            "auto_exclusive" => {
                self.set_auto_exclusive(expect_bool(value)?);
                Ok(())
            }
            "flat" => {
                self.set_flat(expect_bool(value)?);
                Ok(())
            }
            "elide" => {
                let token = expect_string(value)?;
                let Some(mode) = TextElideMode::from_name(&token) else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                self.set_elide_mode(mode);
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
            "drag_enabled" => {
                self.set_drag_enabled(expect_bool(value)?);
                Ok(())
            }
            "accepts_drops" => {
                self.set_accepts_drops(expect_bool(value)?);
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
        // Mirrors `BUTTON_PROPERTIES` for the properties this control owns; the
        // shared four are appended from `BASE_PROPERTY_NAMES` so they are not
        // retyped per control. The menu names exist only on the `full_widgets`
        // surface, which is the only build that has a menu model to attach.
        #[cfg(full_widgets)]
        return property_names_of![
            "text",
            "pressed",
            "default",
            "icon_source",
            "icon_size",
            "shortcut",
            "auto_default",
            "auto_exclusive",
            "flat",
            "elide",
            "drag_enabled",
            "accepts_drops",
            "auto_repeat",
            "auto_repeat_delay",
            "auto_repeat_interval",
            "menu_open",
            "menu_item_count",
            BASE_PROPERTY_NAMES
        ];
        #[cfg(not(full_widgets))]
        return property_names_of![
            "text",
            "pressed",
            "default",
            "icon_source",
            "icon_size",
            "shortcut",
            "auto_default",
            "auto_exclusive",
            "flat",
            "elide",
            "drag_enabled",
            "accepts_drops",
            "auto_repeat",
            "auto_repeat_delay",
            "auto_repeat_interval",
            BASE_PROPERTY_NAMES
        ];
    }

    /// Runs one of the commands `button` publishes.
    ///
    /// `click` is the action a programmatic invocation means: press, release and
    /// emit `clicked`, the same sequence a pointer activation produces. It is
    /// deliberately *not* just `press()` — a caller asking for a click expects the
    /// resulting signal, and `press()` alone only mutates the visual state and emits
    /// nothing.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "click" => {
                self.press();
                self.release();
                self.emit_clicked();
                Ok(())
            }
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for Button {
    /// The four-segment activation contract.
    ///
    /// # Why `released` and `clicked` are not the same event
    ///
    /// A pointer that goes down on a button and comes up away from it is a *canceled*
    /// gesture: the user changed their mind or was dragging something. The rule
    /// is that `clicked` is emitted only when the release lands **inside** the control and
    /// a cancellation is reported otherwise.
    ///
    /// This handler used to treat every release as an activation, so dragging off a
    /// button and letting go still fired it — including for a drag that started
    /// elsewhere and merely ended here.
    ///
    /// | event | result |
    /// |---|---|
    /// | press | `pressed` armed, `pressed` signal emitted |
    /// | move | `pressed` follows whether the pointer is still inside |
    /// | release inside | release, `clicked` |
    /// | release outside | release, `canceled`, **no** `clicked` |
    /// | ungrab (leave / focus loss / disable) | release, `canceled` |
    ///
    /// The whole table applies to a stylus as well as to a mouse (D09-POINTER-01):
    /// `PointerPress`/`PointerMove`/`PointerRelease` take the same arms as their mouse
    /// counterparts, so a pen tap activates the button and a pen drag-off cancels it.
    fn handle_event(&mut self, event: &Event) {
        // Refresh the exclusive group's geometry/enabled snapshot before anything reads it.
        self.sync_group_registration();
        // An open menu owns the next interaction: a click either activates a row or dismisses the
        // popup, and the keyboard drives the highlight instead of the button's activation.
        #[cfg(full_widgets)]
        if self.menu_open {
            let mut consumed = false;
            match event {
                Event::KeyPress { key, .. } if *key == key_codes::ESCAPE => {
                    self.close_menu();
                    consumed = true;
                }
                Event::KeyPress { key, .. } if *key == key_codes::DOWN => {
                    self.move_menu_highlight(true);
                    consumed = true;
                }
                Event::KeyPress { key, .. } if *key == key_codes::UP => {
                    self.move_menu_highlight(false);
                    consumed = true;
                }
                Event::KeyPress { key, .. }
                    if *key == key_codes::ENTER || *key == key_codes::SPACE =>
                {
                    if let Some(index) = self.active_menu_index {
                        self.activate_menu_row(index);
                    }
                    consumed = true;
                }
                Event::MousePress { pos, button, .. } if *button == 1 => {
                    if let Some(index) = self.hit_test_menu_item(*pos) {
                        self.activate_menu_row(index);
                    } else if !self.menu_rect().contains_point(*pos) {
                        self.close_menu();
                    }
                    consumed = true;
                }
                Event::MouseMove { pos } => {
                    if let Some(index) = self.hit_test_menu_item(*pos) {
                        if self.active_menu_index != Some(index) {
                            self.active_menu_index = Some(index);
                            self.base.request_redraw();
                        }
                    }
                }
                _ => { /* Other events still reach the ordinary arms below */ }
            }
            if consumed {
                self.base.handle_event(event);
                return;
            }
        }
        // Gesture logic runs **before** the base records the event. The base both paints and
        // records from the same primitive facts, and it clears `grabbed`/`pressed` on the
        // release — so a control that read them after delegating could no longer tell
        // whether the release belonged to its own gesture. Handling first keeps the arm's
        // guard meaningful, and the base then records the same outcome from the same facts.
        match event {
            Event::MousePress { pos, button, .. } | Event::PointerPress { pos, button, .. }
                if self.base.is_enabled() =>
            {
                // Only a press that resolves to this control arms it. The runtime
                // hit-tests before delivery, but a direct dispatch (a test, a host with
                // its own routing, a designer preview) does not, and an unguarded press
                // would leave the latch armed for a release that belongs elsewhere.
                //
                // A stylus press (`PointerPress`) is a full press here (D09-POINTER-01):
                // before this, a pen tap never armed the button, so it could not activate
                // even though `PointerRelease` already reached the release arm below.
                if button_activates(*button) && self.base.contains_point_with_touch_expansion(*pos)
                {
                    self.press();
                }
            }
            #[cfg(feature = "touch")]
            Event::TouchBegin { .. } if self.base.is_enabled() => {
                self.press();
            }
            // `pressed` is a *continuous* quantity, not an edge: dragging off the
            // control clears it and dragging back restores it, so the button visually
            // commits again. The move handling does this.
            // Without it, a press that wandered off
            // left the button painted pressed forever.
            //
            // The arm is guarded on the **grab**, not on `pressed`: guarding on
            // `pressed` made the move that should restore the state exit early, so a
            // drag out and back could never re-arm.
            Event::MouseMove { pos } | Event::PointerMove { pos, .. } if self.base.is_grabbed() => {
                self.set_pressed(self.base.contains_point_with_touch_expansion(*pos));
            }
            Event::MouseRelease { pos, button, .. } | Event::PointerRelease { pos, button, .. }
                if self.base.is_grabbed() =>
            {
                if !button_activates(*button) {
                    return;
                }
                // Inside ⇒ activate; outside ⇒ cancel. This is the whole contract.
                //
                // A stylus release completes the gesture exactly like a mouse release
                // (D09-POINTER-01), so a pen tap activates the button when it lands inside
                // and cancels when it is dragged off.
                let inside = self.base.contains_point_with_touch_expansion(*pos);
                self.release();
                if inside {
                    self.emit_clicked();
                    #[cfg(full_widgets)]
                    if !self.menu_items.is_empty() {
                        self.open_menu();
                    }
                } else {
                    self.canceled.emit();
                }
            }
            #[cfg(feature = "touch")]
            Event::TouchEnd { pos, .. } if self.base.is_grabbed() => {
                let inside = self.base.contains_point_with_touch_expansion(*pos);
                self.release();
                if inside {
                    self.emit_clicked();
                    #[cfg(full_widgets)]
                    if !self.menu_items.is_empty() {
                        self.open_menu();
                    }
                } else {
                    self.canceled.emit();
                }
            }
            // D09-EVT-02: a withdrawn contact ends the gesture without activating. The
            // platform can normalise a `TouchCancel` to a `TouchEnd`, so the cancel arrives
            // here as its own internal event; reusing `cancel_gesture` gives exactly the
            // "interaction is off" handling (clear the latch and grab, emit `canceled`) and
            // never emits `clicked`.
            #[cfg(feature = "touch")]
            event if crate::event::translator::is_touch_cancel(event) => {
                self.cancel_gesture();
                self.base.request_redraw();
            }
            #[cfg(feature = "touch")]
            Event::Tap { .. } if self.base.is_enabled() => {
                // A tap carries no position: the platform already resolved it to this
                // control, which is the same basis the hit test narrows.
                self.emit_clicked();
                self.state_changed.emit(self.state());
            }
            Event::FocusGained { reason } => {
                let _ = reason;
                // `autoDefault`: the button the keyboard is on is the one Enter presses.
                if self.auto_default && self.base.is_enabled() {
                    self.set_default(true);
                }
                self.base.request_redraw();
            }
            Event::FocusLost => {
                // Focus lost mid-press abandons the gesture: the release belongs to
                // whatever the window switch moved to.
                self.cancel_gesture();
                if self.auto_default {
                    self.set_default(false);
                }
                self.base.request_redraw();
            }
            // A declared shortcut activates the button wherever the host delivers it.
            Event::KeyPress { key, modifiers }
                if self.shortcut_matches(*key, *modifiers) && self.base.is_enabled() =>
            {
                self.click();
            }
            Event::KeyPress { key, .. }
                if is_keyboard_activation(*key) && self.base.is_enabled() =>
            {
                // A keyboard activation is `Shortcut`: the user is on the keyboard, so
                // the ring stays lit. Only the activation itself runs here; the ring
                // follows from the focus reason already stored.
                #[cfg(full_widgets)]
                if !self.menu_items.is_empty() {
                    self.toggle_menu();
                } else {
                    self.click();
                }
                #[cfg(not(full_widgets))]
                self.click();
            }
            Event::MouseEnter { .. } => {
                self.request_frame_repaint(None);
            }
            Event::MouseLeave { .. } => {
                // A pointer that leaves while held abandons the painted pressed state,
                // but the *grab* is kept: `MouseLeave` fires as soon as the pointer
                // crosses the edge, and a drag that comes back should still be able to
                // complete. Only the release, wherever it lands, ends the gesture and
                // decides the outcome.
                self.set_pressed(false);
                self.request_frame_repaint(None);
            }
            _ => { /* Other events are not relevant */ }
        }
        // After the control's own gesture: let the base record the primitive facts
        // (hover, press, grab, focus reason) for the state channel.
        self.base.handle_event(event);
    }
}

/// A dropped button leaves its exclusive group.
///
/// The group registry is thread-local and has no lifetime of its own, so a control that is never
/// unregistered would keep a group alive after it is gone.
impl Drop for Button {
    fn drop(&mut self) {
        // Detaches the control from every group it might belong to: the auto-exclusive group its
        // properties name and the group a `ButtonGroup` handle adopted it into.
        button_group::leave_all(self.id());
    }
}

/// A draggable button carries its own caption.
///
/// `text/plain` is the type the crate's text controls already exchange (see `LineEdit`), so a
/// button dragged onto a field needs no adapter at either end.
impl crate::event::dnd::DragSource for Button {
    fn drag_payload(&self, pos: Point) -> Option<crate::event::dnd::DragPayload> {
        if !self.base.is_enabled() || !self.drag_enabled || self.text.is_empty() {
            return None;
        }
        if !self.base.contains_point_with_touch_expansion(pos) {
            return None;
        }
        Some(
            crate::event::dnd::DragPayload::new("text/plain", self.text.clone())
                .with_label(self.text.clone())
                .with_origin(pos),
        )
    }
}

/// A drop-accepting button takes a plain-text payload as its caption.
///
/// `Copy` rather than `Move`: the payload is text, and the source control's own copy is still
/// there — the button is a second place it can appear, not a container that consumed it.
impl crate::event::dnd::DropTarget for Button {
    fn can_accept(&self, payload: &crate::event::dnd::DragPayload) -> bool {
        payload.is_type("text/plain")
    }

    fn on_drop(
        &mut self,
        payload: &crate::event::dnd::DragPayload,
        _pos: Point,
    ) -> crate::event::dnd::DropEffect {
        let label = if payload.label.is_empty() { &payload.item_id } else { &payload.label };
        self.set_text(label.clone());
        crate::event::dnd::DropEffect::Copy
    }

    fn preview_rect(&self, _payload: &crate::event::dnd::DragPayload, _pos: Point) -> Option<Rect> {
        Some(self.geometry())
    }

    fn report_hover(&mut self, _payload: &crate::event::dnd::DragPayload) {
        if !self.drop_hovered {
            self.drop_hovered = true;
            self.base.request_redraw();
        }
    }

    fn report_leave(&mut self) {
        if self.drop_hovered {
            self.drop_hovered = false;
            self.base.request_redraw();
        }
    }
}

/// Whether `button` is one that activates a control.
///
/// Only the primary button activates. The handler used to accept any button, so a
/// secondary click fired a button's action — and a secondary click is how a host opens
/// a context menu (`runtime.rs` routes `mouse_button::SECONDARY` for exactly that).
/// `CheckBox` and `RadioButton` already guarded on the primary button; this brings
/// `Button` into line rather than being the outlier.
fn button_activates(button: u32) -> bool {
    button == crate::event::mouse_button::PRIMARY
}

/// Whether `key` activates a focused button.
///
/// Space and Enter are the two the platform convention assigns, and they are named
/// here rather than left as literals at the match arm: the raw numbers appeared in
/// every binary control with slightly different spellings (`13` here, `13 | 10`
/// elsewhere), so "which keys activate a button" had no single answer to read.
fn is_keyboard_activation(key: u32) -> bool {
    matches!(key, 13 | 10 | 32)
}
impl Draw for Button {
    fn draw(&mut self, context: &mut RenderContext) {
        // Button rendering with style integration (BLUE13 R1.3)
        // Reads style fields (background_color, text_color, border_color, etc.)
        // falling back to state-based defaults when style values are not set.
        let rect = self.geometry();
        let state = self.state();
        let style = self.style();

        // ── The box actually painted ──
        //
        // A button fills the width it was *given* — that is what a button is, and a wider
        // button is still a button. But its **height** is its own: a 240x120 census cell drew a
        // 240x120 pill, which is a rectangle shaped like a button rather than a button. The
        // height comes from `size_hint`, the same derivation a layout asks for, so the drawn
        // shape and the reported size cannot disagree.
        let intrinsic = self.size_hint();
        let painted_height = intrinsic.height.min(rect.height).max(1);
        let rect = Rect::new(
            rect.x,
            rect.y + (rect.height.saturating_sub(painted_height) / 2) as i32,
            rect.width,
            painted_height,
        );

        // ── Background ──
        let bg = style.background_color.unwrap_or_else(|| match state {
            ButtonState::Normal => Color::rgb(240, 240, 240),
            ButtonState::Pressed => Color::rgb(200, 200, 200),
            ButtonState::Disabled => Color::rgb(220, 220, 220),
        });
        // The fill blends from the resting colour toward the interactive one by the transition's own
        // progress, so a hover fades in and a press deepens it rather than both snapping. The
        // resting and pressed colours are the ones this control already used; only the *path*
        // between them is new.
        //
        // At progress `0.0` — a fresh control, or one whose transition has settled at rest — the
        // result is exactly the resting colour, so a snapshot taken without ticking is unchanged.
        let progress = self.interaction_progress.value();
        let bg = if progress <= 0.0 {
            bg
        } else {
            let interactive = match state {
                ButtonState::Pressed => bg,
                _ => bg.blend(&bg.contrast_color(), 0.22),
            };
            bg.blend(&interactive, progress)
        };
        // The corner radius is a profile shape token, not a second colour fact. A radius the
        // caller or the theme actually chose is honoured; the Material baseline is the generic
        // theme radius rather than a button-specific choice, so it follows the active profile
        // instead — 6 px on macOS, Material's own 4 everywhere else. An unset radius keeps
        // drawing square, which is what a control with no theme has always done.
        let material_baseline = profile_metrics(ControlMetricsProfile::Material).button_radius;
        let profile = profile_metrics(active_profile());
        let br = match style.border_radius {
            Some(radius) if radius != material_baseline => radius,
            Some(_) => profile.button_radius,
            None => 0,
        };
        // ── The face ──
        //
        // The fill, the bevel and the outline are painted in one call, because they are one
        // operation with an order that matters (fill, then bevel on the boundary, then the
        // outermost line). The surface comes from the style — the role table's default for a
        // button, as any theme override replaced it — so a theme that says "raise this face" or
        // "flatten it" changes what a button is drawn as **without a line of code here**
        // (BLUE24 §10A).
        //
        // `solid()` is the identity, so a button on the flat preset emits exactly the fill and
        // outline it emitted before surfaces existed: the snapshot suite is the proof.
        let surface = style.surface.unwrap_or_else(crate::render::SurfaceStyle::solid);
        if self.flat {
            // A flat button shows no face of its own. A background the **caller** declared is still
            // honoured (that is an explicit request), and the interaction wash is painted so the
            // control acknowledges the pointer — but the theme's own role fill is chrome, which is
            // exactly what `flat` removes.
            let caller_background = !style.theme_derived
                && (style.background_color.is_some() || style.background_gradient.is_some());
            if caller_background {
                context.face_with_gradient(
                    rect,
                    bg,
                    style.background_gradient.as_ref(),
                    surface,
                    br,
                    Color::BLACK,
                );
            } else if progress > 0.0 {
                let interactive = bg.blend(&bg.contrast_color(), 0.22);
                context.fill_rounded_rect(rect, br, interactive);
            }
            if let (Some(color), Some(width)) = (style.border_color, style.border_width) {
                if width > 0 {
                    context.draw_rounded_rect_stroke(rect, br, color, width);
                }
            }
        } else {
            surface.paint_with_gradient(
                context,
                rect,
                bg,
                style.background_gradient.as_ref(),
                style.border_color.unwrap_or(bg),
                style.border_width.unwrap_or(0),
                br,
                Color::BLACK,
            );
        }

        // ── Icon ──
        //
        // The icon is the **leading** content slot, so its box comes from `icon_rect` — the same
        // derivation `implicit_size` reserves room with — rather than from a second, hand-written
        // offset. §B.9's point is exactly this: a wider icon pushes the label along instead of
        // overlapping it.
        let icon_color = style.text_color.unwrap_or_else(|| {
            let fill = style.background_color.unwrap_or(Color::WHITE);
            if state == ButtonState::Disabled {
                fill.contrast_color().with_alpha(150)
            } else {
                fill.contrast_color()
            }
        });
        if let (Some(source), Some(box_)) = (self.icon_source, self.icon_rect()) {
            if let Err(error) = crate::widget::paint_icon(context, source.into(), box_, icon_color)
            {
                log::warn!("button icon could not be painted: {error:?}");
            }
        }

        #[cfg(feature = "image")]
        if self.icon_source.is_none() {
            #[cfg(not(alloc_frugal))]
            if let (Some(pixels), Some(box_)) = (self.shared_icon_pixels.as_ref(), self.icon_rect())
            {
                context.draw_image(box_.x, box_.y, box_.width, box_.height, pixels.pixels());
            } else if let (Some(icon), Some(box_)) = (self.icon.as_ref(), self.icon_rect()) {
                // The icon is scaled to the slot the metric derivation reserved for it, so the room
                // `implicit_size` added and the pixels drawn are the same square. An icon whose own
                // bitmap is a different size is scaled rather than drawn at its native extent —
                // otherwise the button's reserved advance and its ink would disagree.
                context.draw_image(
                    box_.x,
                    box_.y,
                    box_.width,
                    box_.height,
                    icon.rgba8_data().unwrap_or(&[]),
                );
            }
            #[cfg(alloc_frugal)]
            if let (Some(icon), Some(box_)) = (self.icon.as_ref(), self.icon_rect()) {
                // The icon is scaled to the slot the metric derivation reserved for it, so the room
                // `implicit_size` added and the pixels drawn are the same square. An icon whose own
                // bitmap is a different size is scaled rather than drawn at its native extent —
                // otherwise the button's reserved advance and its ink would disagree.
                context.draw_image(
                    box_.x,
                    box_.y,
                    box_.width,
                    box_.height,
                    icon.rgba8_data().unwrap_or(&[]),
                );
            }
        }

        // ── Text ──
        if !self.text.is_empty() {
            // The draw path and `implicit_size` take their font from the same accessor, so the
            // label measured and the label painted cannot disagree (D09-STYLE-02).
            let font = effective_font(style);
            // The ink must be legible *on the fill this button actually got*, not on an
            // assumption about it. The fill comes from `style.background_color`, which the
            // theme may have set to a saturated accent: the dark appearance's `PRIMARY` is
            // `rgb(100,181,246)`, and the old rule drew pure black on it — chosen from
            // `state == Disabled` alone, with no reference to the background at all, so
            // `button.svg` carried a black label on a light-blue pill. Deriving the ink from
            // the resolved fill is what makes "one foreground per background" true for any
            // theme, which is the same rule `badge` and `role_colors` already use.
            let fill = style.background_color.unwrap_or(Color::WHITE);
            let text_color = style.text_color.unwrap_or_else(|| {
                if state == ButtonState::Disabled {
                    // A disabled button's ink is *deliberately* low-contrast: that is what
                    // makes it read as unavailable. It is still derived from the fill's own
                    // luminance so it recedes in the dark appearance too instead of
                    // disappearing into it.
                    fill.contrast_color().with_alpha(150)
                } else {
                    fill.contrast_color()
                }
            });
            // Vertically centred through the shared primitive, so the label sits in the
            // button's middle instead of having its glyph-box top edge on that middle line.
            let line = context.text_line(rect, font);
            // Horizontally centred in the button's own box, which is what a push button's
            // label is: a push button's `contentItem` is an `AbstractButton`
            // whose `Text` is horizontally centred, and Material M3's
            // `TextButton`/`ElevatedButton` centre their child the same way. The label used to
            // start at `rect.x + BUTTON_PADDING_H`, i.e. flush to the left inside a pill many
            // times its own width — the label read as a left-aligned caption rather than as
            // the button's own name, and `snapshots/svg/button.svg` showed it at x = 12 in a
            // 240-wide control while `toggle_button` and `tool_button` centred theirs.
            //
            // `BUTTON_PADDING_H` is still the right constant here, but as the **minimum**
            // inset from either edge rather than as a left anchor: it is what keeps a label
            // that nearly fills the button from touching the border, and it is the same
            // number `size_hint` adds to the content when deriving the intrinsic width, so the
            // drawn label and the reported size cannot disagree. Centring inside the padded
            // box, rather than inside the raw rectangle, is what gives a wide button the room
            // the constant promises.
            // The label's own box comes from `label_rect`, which is the content box with the
            // icon's advance removed. That is the trailing slot of the same two-part row, so a
            // button with an icon centres its label in the room *after* the icon rather than under
            // it. Without an icon the advance is zero and this is the whole content box.
            let label_box = self.label_rect();
            let label_bounds = Rect::new(label_box.x, line.y, label_box.width, line.height);
            match self.elide_mode {
                // `ElideRight` is the historical fitted draw, byte for byte.
                TextElideMode::ElideRight => {
                    context.draw_text_fitted_cow(
                        label_bounds,
                        &self.text,
                        font,
                        text_color,
                        HorizontalAlignment::Center,
                    );
                }
                // `None` paints the whole string at the aligned origin, which is the "let the
                // renderer clip" behaviour a caller asks for explicitly.
                TextElideMode::None => {
                    let width = context.measure_text(&self.text, font).width as i32;
                    let x = label_bounds.x + (label_bounds.width as i32 - width).max(0) / 2;
                    context.draw_text(
                        Point::new(x, label_bounds.y),
                        &self.text,
                        font,
                        text_color,
                        HorizontalAlignment::Left,
                    );
                }
                mode => draw_elided_line(
                    context,
                    label_bounds,
                    &self.text,
                    font,
                    text_color,
                    HorizontalAlignment::Center,
                    mode,
                ),
            }
        }

        // ── Default-button emphasis ──
        //
        // A default button is the one Enter activates, and every desktop convention says so *on
        // screen* -- a heavier outline, or the accent. The flag was stored, published and toggled by
        // the schema while `draw` never read it, so `set_default(true)` redrew an identical button
        // and the user had no way to see which one Enter would press.
        //
        // It is drawn **between** the border and the focus ring, and inside the control's own
        // rectangle, for the same reason the focus ring is: a ring outside the rect is the
        // "drawing leaves the control and only the raster clip hides it" defect the census catches.
        // The colour is the theme's accent -- the emphasis has to be visible on a button whose
        // surface and border the host may have styled arbitrarily, and `style.background_color` is
        // exactly the resolved value the accent already lands in for an active/primary button.
        if self.is_default() {
            let ring = FocusRing::for_control(rect, br);
            if ring.is_drawable() {
                context.draw_rounded_rect_stroke(
                    ring.rect,
                    ring.radius,
                    bg.contrast_color(),
                    DEFAULT_EMPHASIS_WIDTH,
                );
            }
        }

        // ── Focus ring ──
        //
        // Drawn strictly inside the control's rectangle (see `ControlMetrics::focus_ring_rect`)
        // and only when the *reason* focus arrived warrants it: a pointer press focuses without
        // drawing a ring, Tab and Shortcut draw one. This is the standard rule
        // for visual focus, and it is why `Button` carries a `FocusReason` rather than
        // a bare `focused` bool.
        //
        // The macOS metrics profile asks for the platform's outer accent halo instead of the inset
        // stroke. Both are selected here so the two styles cannot be mixed up: every other
        // profile keeps the inset ring byte for byte, which is what preserves the Material
        // snapshot.
        if self.visual_focus() {
            if active_profile() == ControlMetricsProfile::Macos {
                let halo = FocusRing::halo_for_control(rect, br);
                if halo.is_drawable() {
                    context.draw_box_shadow(
                        halo.rect,
                        focus_ring_color(bg.contrast_color()),
                        0,
                        0,
                        FOCUS_HALO_BLUR_RADIUS,
                        1,
                    );
                }
            } else {
                let ring = FocusRing::for_control(rect, br);
                if ring.is_drawable() {
                    context.draw_rounded_rect_stroke(
                        ring.rect,
                        ring.radius,
                        focus_ring_color(bg.contrast_color()),
                        FOCUS_RING_WIDTH,
                    );
                }
            }
        }

        // ── Drop feedback ──
        //
        // Only reachable when the caller opted in to drops; a plain button is unchanged.
        if self.drop_hovered {
            context.draw_rounded_rect_stroke(rect, br, focus_ring_color(bg.contrast_color()), 2);
        }

        // ── Attached menu ──
        //
        // Painted after the button's own chrome because it sits outside its rectangle. The
        // geometry comes from `menu_rect`, the same derivation the hit test uses, so a row is
        // clickable exactly where it was drawn.
        #[cfg(full_widgets)]
        if self.menu_open {
            let panel = self.menu_rect();
            let accent = crate::style::palette_shared()
                .as_ref()
                .map(|theme| theme.accent)
                .unwrap_or_else(|| bg.contrast_color());
            draw_menu_panel(context, panel, &self.menu_items, &[], style, accent);
            // The keyboard highlight is the button's own chrome, so it is painted here rather
            // than being a property of the shared menu painter (which the menu button owns).
            if let Some(index) = self.active_menu_index {
                if self.menu_items.get(index).is_some_and(|item| item.enabled) {
                    let row = menu_item_rect(panel, index);
                    context.draw_rounded_rect_stroke(row, 2, accent, 1);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Color, Point, Rect, Size};
    use crate::event::Event;
    #[cfg(all(feature = "image", not(alloc_frugal)))]
    use crate::widget::Image;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    // ── Helper ─────────────────────────────────────────────────────────
    fn make_button() -> Button {
        Button::new("Click".into(), Rect::new(10, 20, 120, 36))
    }

    #[cfg(all(feature = "image", not(alloc_frugal)))]
    fn make_image() -> Image {
        Image::try_from_rgba(vec![0u8; 8 * 8 * 4], 8, 8)
            .expect("the buffer matches its declared size")
    }

    fn rect() -> Rect {
        Rect::new(10, 20, 120, 36)
    }

    // ── Property contract (BLUE15 C-1) ─────────────────────────────────

    /// `Button`'s own properties must round-trip through the new contract.
    ///
    /// This is the reference test for every other control's migration: it proves
    /// the read path, the write path and the published name list describe the same
    /// properties, which the centralised `match kind()` dispatch could not
    /// guarantee (a name could be writable and unreadable, or listed and absent).
    #[test]
    fn widget_properties_round_trip() {
        let mut button = make_button();

        assert_eq!(button.get("text"), Ok(CapabilityValue::String("Click".into())));
        button.set("text", CapabilityValue::String("Go".into())).expect("text is writable");
        assert_eq!(button.get("text"), Ok(CapabilityValue::String("Go".into())));

        assert_eq!(button.get("pressed"), Ok(CapabilityValue::Bool(false)));
        button.set("pressed", CapabilityValue::Bool(true)).expect("pressed is writable");
        assert_eq!(button.get("pressed"), Ok(CapabilityValue::Bool(true)));

        button.set("default", CapabilityValue::Bool(true)).expect("default is writable");
        assert_eq!(button.get("default"), Ok(CapabilityValue::Bool(true)));

        let icon = crate::widget::icon_source::IconSpec::Builtin(crate::widget::IconName::Info);
        button
            .set("icon_source", CapabilityValue::IconSpec(icon.clone()))
            .expect("built-in icon descriptors are applicable without a resolver");
        assert_eq!(button.get("icon_source"), Ok(CapabilityValue::IconSpec(icon)));
        button.set("icon_source", CapabilityValue::Null).expect("null clears the icon");
        assert_eq!(button.get("icon_source"), Ok(CapabilityValue::Null));
        assert_eq!(
            button.set(
                "icon_source",
                CapabilityValue::IconSpec(crate::widget::icon_source::IconSpec::Resource {
                    id: "project/icons/save".into(),
                    color_mode: crate::widget::icon_source::IconColorMode::Monochrome,
                }),
            ),
            Err(CapabilityAccessError::ResourceResolutionRequired)
        );
        static DATA: crate::widget::display_widgets::icon::IconData =
            crate::widget::display_widgets::icon::IconData {
                name: "runtime",
                grid: 24,
                paths: &[],
            };
        button.set_icon_source(Some(IconSource::static_geometry(&DATA)));
        assert_eq!(button.get("icon_source"), Err(CapabilityAccessError::IconSourceNotPersistable));
    }

    /// The shared properties must work through `Button` too, not only through the
    /// base helpers directly — that forwarding is what a migration can silently
    /// break.
    #[test]
    fn shared_properties_are_forwarded_to_the_base_contract() {
        let mut button = make_button();

        assert_eq!(button.get("enabled"), Ok(CapabilityValue::Bool(true)));
        button.set("enabled", CapabilityValue::Bool(false)).expect("enabled is writable");
        assert!(!button.is_enabled(), "the base state must actually change");

        assert_eq!(button.get("geometry"), Ok(CapabilityValue::String("10,20,120,36".into())));
    }

    /// Every name `Button` publishes must be readable, and its own names must all
    /// be present — a `property_names` that omits a property makes it invisible to
    /// schema consumers even though `get` would answer.
    #[test]
    fn published_names_match_the_contract() {
        let button = make_button();
        let names = button.property_names();

        for required in ["text", "pressed", "default"] {
            assert!(names.contains(&required), "property_names must publish {required:?}");
            assert!(button.get(required).is_ok(), "published {required:?} must be readable");
        }
    }

    // ── 1. Button creation ─────────────────────────────────────────────
    #[cfg(feature = "image")]
    #[test]
    fn button_creation_text_geometry_defaults_icon() {
        let b = make_button();
        assert_eq!(b.text(), "Click");
        assert_eq!(b.geometry(), rect());
        assert_eq!(b.state(), ButtonState::Normal);
        assert!(!b.is_pressed());
        assert!(!b.is_default());
        assert!(b.icon().is_none());
    }

    // ── 2. State transitions ───────────────────────────────────────────
    #[test]
    fn state_transition_normal_pressed_released() {
        let mut b = make_button();
        assert_eq!(b.state(), ButtonState::Normal);

        b.press();
        assert!(b.is_pressed());
        assert_eq!(b.state(), ButtonState::Pressed);

        b.release();
        assert!(!b.is_pressed());
        assert_eq!(b.state(), ButtonState::Normal);
    }

    #[test]
    fn state_transition_idempotent_press_release_noop() {
        let mut b = make_button();
        // Already Normal → release is a no-op
        b.release();
        assert_eq!(b.state(), ButtonState::Normal);
        assert!(!b.is_pressed());

        // Press once
        b.press();
        // Second press is a no-op
        b.press();
        assert!(b.is_pressed());
        assert_eq!(b.state(), ButtonState::Pressed);

        // Release once
        b.release();
        // Second release is a no-op
        b.release();
        assert!(!b.is_pressed());
        assert_eq!(b.state(), ButtonState::Normal);
    }

    // ── 3. Signal emission ─────────────────────────────────────────────
    #[test]
    fn signal_press_emits_pressed_and_state_changed() {
        let mut b = make_button();
        let pressed_fired = Arc::new(AtomicBool::new(false));
        let changed_fired = Arc::new(AtomicBool::new(false));
        b.pressed_signal.connect({
            let flag = Arc::clone(&pressed_fired);
            move || {
                flag.store(true, Ordering::SeqCst);
            }
        });
        b.state_changed.connect({
            let flag = Arc::clone(&changed_fired);
            move |_| {
                flag.store(true, Ordering::SeqCst);
            }
        });

        b.press();

        assert!(pressed_fired.load(Ordering::SeqCst));
        assert!(changed_fired.load(Ordering::SeqCst));
    }

    #[test]
    fn signal_release_emits_released_and_state_changed() {
        let mut b = make_button();
        let released_fired = Arc::new(AtomicBool::new(false));
        let changed_fired = Arc::new(AtomicBool::new(false));
        b.released_signal.connect({
            let flag = Arc::clone(&released_fired);
            move || {
                flag.store(true, Ordering::SeqCst);
            }
        });
        b.state_changed.connect({
            let flag = Arc::clone(&changed_fired);
            move |_| {
                flag.store(true, Ordering::SeqCst);
            }
        });

        b.press(); // Press first
        released_fired.store(false, Ordering::SeqCst);
        changed_fired.store(false, Ordering::SeqCst);

        b.release(); // Then release

        assert!(released_fired.load(Ordering::SeqCst));
        assert!(changed_fired.load(Ordering::SeqCst));
    }

    #[test]
    fn set_enabled_state_requests_redraw() {
        let mut b = make_button();
        let fired = Arc::new(AtomicBool::new(false));
        b.base.redraw_requested.connect({
            let flag = Arc::clone(&fired);
            move || {
                flag.store(true, Ordering::SeqCst);
            }
        });

        b.set_enabled_state(false);

        assert!(fired.load(Ordering::SeqCst));
    }

    #[test]
    fn signal_no_emission_on_noop_transition() {
        let mut b = make_button();
        let fired = Arc::new(AtomicBool::new(false));
        b.pressed_signal.connect({
            let flag = Arc::clone(&fired);
            move || {
                flag.store(true, Ordering::SeqCst);
            }
        });
        b.released_signal.connect({
            let flag = Arc::clone(&fired);
            move || {
                flag.store(true, Ordering::SeqCst);
            }
        });
        b.state_changed.connect({
            let flag = Arc::clone(&fired);
            move |_| {
                flag.store(true, Ordering::SeqCst);
            }
        });

        // No-op: Normal → Normal
        b.release();
        assert!(!fired.load(Ordering::SeqCst));

        // Press → Pressed
        b.press();
        fired.store(false, Ordering::SeqCst);

        // No-op: Pressed → Pressed
        b.press();
        assert!(!fired.load(Ordering::SeqCst));
    }

    // ── 4. Disabled state ──────────────────────────────────────────────
    #[test]
    fn disabled_prevents_transitions() {
        let mut b = make_button();
        b.set_enabled_state(false);
        assert_eq!(b.state(), ButtonState::Disabled);

        // Press is ignored when disabled
        b.press();
        assert_eq!(b.state(), ButtonState::Disabled);
        assert!(!b.is_pressed());

        // Release is ignored when disabled
        b.release();
        assert_eq!(b.state(), ButtonState::Disabled);
    }

    #[test]
    fn re_enable_restores_normal_state() {
        let mut b = make_button();
        b.set_enabled_state(false);
        assert_eq!(b.state(), ButtonState::Disabled);

        b.set_enabled_state(true);
        assert_eq!(b.state(), ButtonState::Normal);
        assert!(!b.is_pressed());
    }

    // ── 5. Text update and icon ────────────────────────────────────────
    #[test]
    fn set_text_updates_text() {
        let mut b = make_button();
        assert_eq!(b.text(), "Click");

        b.set_text("OK");
        assert_eq!(b.text(), "OK");
    }

    #[test]
    fn set_empty_text() {
        let mut b = make_button();
        b.set_text("");
        assert_eq!(b.text(), "");
        assert!(b.text().is_empty());
    }

    #[test]
    fn typed_icon_source_owns_layout_and_clears_cleanly() {
        let mut button = make_button();
        assert!(button.icon_rect().is_none());
        let source = IconSource::builtin(crate::widget::IconName::Info);
        button.set_icon_source(Some(source));
        assert_eq!(button.icon_source(), Some(source));
        assert!(button.icon_rect().is_some());
        assert!(
            button.implicit_size().width
                > Button::new("Click".into(), rect()).implicit_size().width,
            "typed geometry reserves the same content slot as a legacy image"
        );
        #[cfg(feature = "image")]
        assert!(button.icon().is_none(), "typed content is not exposed as a legacy image");

        button.set_icon_source(None);
        assert_eq!(button.icon_source(), None);
        assert!(button.icon_rect().is_none(), "clearing releases the icon layout slot");
    }

    #[cfg(all(feature = "image", not(alloc_frugal)))]
    #[test]
    fn shared_icon_pixels_replace_other_sources_and_keep_the_shared_allocation() {
        let handle = crate::widget::SharedIconPixels::from_rgba8(vec![255; 4 * 4 * 4], 4, 4)
            .expect("valid RGBA8 data");
        let clone = handle.clone();
        assert!(handle.shares_allocation_with(&clone));

        let mut button = make_button();
        button.set_shared_icon_pixels(Some(handle.clone()));
        assert!(button.icon().is_none());
        assert!(button.shared_icon_pixels().unwrap().shares_allocation_with(&handle));
        assert!(button.icon_rect().is_some());
        let svg = crate::widget::svg::render_widget_to_svg(&mut button, rect());
        assert!(svg.contains("<image "), "shared in-memory pixels must reach the paint backend");
        let icon_box = button.icon_rect();
        button.set_enabled_state(false);
        assert_eq!(button.icon_rect(), icon_box, "disabled state must not shift the icon slot");

        button.set_icon_source(Some(IconSource::builtin(crate::widget::IconName::Info)));
        assert!(button.shared_icon_pixels().is_none());
        assert!(button.icon_rect().is_some());
        button.set_shared_icon_pixels(None);
        assert!(button.icon_source().is_none());
        assert!(button.icon_rect().is_none());

        button.set_shared_icon_pixels(Some(handle));
        button.set_icon(make_image());
        assert!(button.shared_icon_pixels().is_none());
        assert!(button.icon().is_some(), "the legacy image setter becomes the active source");
    }

    #[cfg(all(feature = "image", not(alloc_frugal)))]
    #[test]
    fn shared_pixels_scale_to_many_consumers_and_release_with_the_last_control() {
        let allocation = Arc::new(vec![128; 4]);
        let weak = Arc::downgrade(&allocation);
        let handle =
            crate::widget::SharedIconPixels::from_shared_rgba8(Arc::clone(&allocation), 1, 1)
                .expect("valid one-pixel RGBA image");
        let mut consumers = Vec::with_capacity(100);
        for _ in 0..100 {
            let mut button = make_button();
            button.set_shared_icon_pixels(Some(handle.clone()));
            assert!(button
                .shared_icon_pixels()
                .expect("shared source")
                .shares_allocation_with(&handle));
            consumers.push(button);
        }

        drop(allocation);
        drop(handle);
        assert!(weak.upgrade().is_some(), "live controls must retain the shared allocation");
        drop(consumers);
        assert!(weak.upgrade().is_none(), "dropping the final control must release the pixels");
    }

    #[cfg(all(feature = "image", not(alloc_frugal)))]
    #[test]
    fn set_icon_and_default_icon() {
        let mut b = make_button();
        assert!(b.icon().is_none());

        b.set_icon_source(Some(IconSource::builtin(crate::widget::IconName::Info)));
        let img = make_image();
        b.set_icon(img);
        assert!(b.icon().is_some());
        assert_eq!(b.icon_source(), None, "legacy image setter replaces typed content");
    }

    // ── 6. Default property ────────────────────────────────────────────
    #[test]
    fn default_property() {
        let mut b = make_button();
        assert!(!b.is_default());

        b.set_default(true);
        assert!(b.is_default());

        b.set_default(false);
        assert!(!b.is_default());
    }

    // ── 7. Event handling ──────────────────────────────────────────────
    #[test]
    fn focus_gained_sets_focused_flag() {
        let mut btn = make_button();
        assert!(!btn.is_focused(), "Button should not be focused by default");
        btn.handle_event(&Event::FocusGained { reason: FocusReason::Programmatic });
        assert!(btn.is_focused(), "Button should be focused after FocusGained");
    }

    #[test]
    fn focus_lost_clears_focused_flag() {
        let mut btn = make_button();
        btn.handle_event(&Event::FocusGained { reason: FocusReason::Programmatic });
        assert!(btn.is_focused());
        btn.handle_event(&Event::FocusLost);
        assert!(!btn.is_focused(), "Button should not be focused after FocusLost");
    }

    #[test]
    fn key_press_enter_triggers_click() {
        let mut btn = make_button();
        let clicked = Arc::new(AtomicBool::new(false));
        btn.base.clicked.connect({
            let flag = Arc::clone(&clicked);
            move || {
                flag.store(true, Ordering::SeqCst);
            }
        });
        btn.handle_event(&Event::KeyPress { key: 13, modifiers: 0 });
        assert!(clicked.load(Ordering::SeqCst), "Enter key should trigger click");
    }

    #[test]
    fn key_press_space_triggers_click() {
        let mut btn = make_button();
        let clicked = Arc::new(AtomicBool::new(false));
        btn.base.clicked.connect({
            let flag = Arc::clone(&clicked);
            move || {
                flag.store(true, Ordering::SeqCst);
            }
        });
        btn.handle_event(&Event::KeyPress { key: 32, modifiers: 0 });
        assert!(clicked.load(Ordering::SeqCst), "Space key should trigger click");
    }

    #[test]
    fn key_press_other_key_does_not_trigger_click() {
        let mut btn = make_button();
        let clicked = Arc::new(AtomicBool::new(false));
        btn.base.clicked.connect({
            let flag = Arc::clone(&clicked);
            move || {
                flag.store(true, Ordering::SeqCst);
            }
        });
        btn.handle_event(&Event::KeyPress { key: 65, modifiers: 0 }); // 'A'
        assert!(!clicked.load(Ordering::SeqCst), "Non-Enter/Space key should NOT trigger click");
    }

    #[test]
    fn mouse_enter_sets_hovered() {
        let mut btn = Button::new("OK".to_string(), Rect::new(0, 0, 80, 30));
        assert!(!btn.is_hovered());
        btn.handle_event(&Event::MouseEnter { pos: Point::new(1, 1) });
        assert!(btn.is_hovered());
    }

    #[test]
    fn mouse_leave_clears_hovered() {
        let mut btn = Button::new("OK".to_string(), Rect::new(0, 0, 80, 30));
        btn.handle_event(&Event::MouseEnter { pos: Point::new(1, 1) });
        assert!(btn.is_hovered());
        btn.handle_event(&Event::MouseLeave { pos: Point::new(1, 1) });
        assert!(!btn.is_hovered());
    }

    #[test]
    fn event_mouse_down_presses_button() {
        let mut b = make_button();
        // `mouse_button::PRIMARY`, not a bare `1`: the activation guard reads the named
        // code, and a test that spelled its own number would not notice the guard moving.
        let event = Event::MousePress {
            pos: crate::core::Point::new(15, 25),
            button: crate::event::mouse_button::PRIMARY,
            modifiers: 0,
        };
        b.handle_event(&event);
        assert!(b.is_pressed());
        assert_eq!(b.state(), ButtonState::Pressed);
    }

    #[test]
    fn event_mouse_up_releases_and_clicks() {
        let mut b = make_button();
        let clicked = Arc::new(AtomicBool::new(false));
        b.base.clicked.connect({
            let flag = Arc::clone(&clicked);
            move || {
                flag.store(true, Ordering::SeqCst);
            }
        });

        // Must be pressed first
        b.press();
        assert!(b.is_pressed());

        let event = Event::MouseRelease {
            pos: crate::core::Point::new(15, 25),
            button: crate::event::mouse_button::PRIMARY,
        };
        b.handle_event(&event);
        assert!(!b.is_pressed());
        assert_eq!(b.state(), ButtonState::Normal);
        assert!(clicked.load(Ordering::SeqCst));
    }

    /// D09-POINTER-01: a stylus press/release activates the button exactly like a mouse.
    ///
    /// Before the fix the button only armed on `MousePress`, so a `PointerPress` armed
    /// nothing and the `PointerRelease` arm (guarded on the grab) was unreachable. The
    /// assertions cover the whole lifecycle: press arms, the release inside clicks, and the
    /// released latch is cleared.
    #[test]
    fn event_pointer_press_and_release_activate_the_button() {
        let mut b = make_button();
        let clicks = Arc::new(core::sync::atomic::AtomicUsize::new(0));
        b.base.clicked.connect({
            let counter = Arc::clone(&clicks);
            move || {
                counter.fetch_add(1, Ordering::SeqCst);
            }
        });
        let canceled = Arc::new(AtomicBool::new(false));
        b.canceled.connect({
            let flag = Arc::clone(&canceled);
            move || flag.store(true, Ordering::SeqCst)
        });

        b.handle_event(&Event::pointer_press(
            Point::new(15, 25),
            crate::event::mouse_button::PRIMARY,
            0.8,
            0.0,
            0.0,
        ));
        assert!(b.is_pressed(), "a stylus press must arm the button like a mouse press");
        assert_eq!(b.state(), ButtonState::Pressed);

        b.handle_event(&Event::pointer_release(
            Point::new(15, 25),
            crate::event::mouse_button::PRIMARY,
            0.7,
        ));
        assert!(!b.is_pressed(), "the stylus release must clear the pressed latch");
        assert_eq!(b.state(), ButtonState::Normal);
        assert_eq!(clicks.load(Ordering::SeqCst), 1, "a stylus tap inside must click once");
        assert!(!canceled.load(Ordering::SeqCst), "a release inside must not cancel");
    }

    /// D09-POINTER-01: dragging a stylus off the button and releasing cancels, mirroring the
    /// mouse contract — a pen drag-off must not activate.
    #[test]
    fn a_pointer_release_outside_the_button_cancels() {
        let mut b = make_button();
        let clicks = Arc::new(core::sync::atomic::AtomicUsize::new(0));
        b.base.clicked.connect({
            let counter = Arc::clone(&clicks);
            move || {
                counter.fetch_add(1, Ordering::SeqCst);
            }
        });
        let canceled = Arc::new(AtomicBool::new(false));
        b.canceled.connect({
            let flag = Arc::clone(&canceled);
            move || flag.store(true, Ordering::SeqCst)
        });

        b.handle_event(&Event::pointer_press(
            Point::new(20, 30),
            crate::event::mouse_button::PRIMARY,
            0.9,
            0.0,
            0.0,
        ));
        // The pen moves off the control before lifting: `pressed` clears, the grab survives.
        b.handle_event(&Event::pointer_move(Point::new(400, 400), 0.9, 0.0, 0.0));
        assert!(!b.is_pressed(), "off the control the painted pressed state clears");

        b.handle_event(&Event::pointer_release(
            Point::new(400, 400),
            crate::event::mouse_button::PRIMARY,
            0.0,
        ));
        assert_eq!(clicks.load(Ordering::SeqCst), 0, "a release outside must not click");
        assert!(canceled.load(Ordering::SeqCst), "a release outside must cancel");
    }

    // ── The activation contract (P0-6 / P0-7) ────────────────────────────

    #[test]
    fn a_release_outside_the_button_cancels_instead_of_clicking() {
        // The defect this guards: every release used to activate, so dragging off a
        // button and letting go still fired it.
        let mut b = make_button();
        let clicked = Arc::new(AtomicBool::new(false));
        let canceled = Arc::new(AtomicBool::new(false));
        b.base.clicked.connect({
            let flag = Arc::clone(&clicked);
            move || flag.store(true, Ordering::SeqCst)
        });
        b.canceled.connect({
            let flag = Arc::clone(&canceled);
            move || flag.store(true, Ordering::SeqCst)
        });

        b.handle_event(&Event::MousePress {
            pos: crate::core::Point::new(20, 30),
            button: crate::event::mouse_button::PRIMARY,
            modifiers: 0,
        });
        assert!(b.is_pressed());

        b.handle_event(&Event::MouseRelease {
            pos: crate::core::Point::new(900, 900),
            button: crate::event::mouse_button::PRIMARY,
        });
        assert!(!b.is_pressed(), "the press ends either way");
        assert!(canceled.load(Ordering::SeqCst), "a release outside must cancel");
        assert!(!clicked.load(Ordering::SeqCst), "a release outside must not click");
    }

    #[test]
    fn a_release_inside_the_button_clicks_and_does_not_cancel() {
        let mut b = make_button();
        let clicked = Arc::new(AtomicBool::new(false));
        let canceled = Arc::new(AtomicBool::new(false));
        b.base.clicked.connect({
            let flag = Arc::clone(&clicked);
            move || flag.store(true, Ordering::SeqCst)
        });
        b.canceled.connect({
            let flag = Arc::clone(&canceled);
            move || flag.store(true, Ordering::SeqCst)
        });

        b.handle_event(&Event::MousePress {
            pos: crate::core::Point::new(20, 30),
            button: crate::event::mouse_button::PRIMARY,
            modifiers: 0,
        });
        b.handle_event(&Event::MouseRelease {
            pos: crate::core::Point::new(30, 40),
            button: crate::event::mouse_button::PRIMARY,
        });
        assert!(clicked.load(Ordering::SeqCst));
        assert!(!canceled.load(Ordering::SeqCst), "a release inside must not cancel");
    }

    #[test]
    fn pressed_is_continuous_as_the_pointer_moves_out_and_back() {
        // `pressed` is not an edge: dragging off clears it, dragging back restores it.
        let mut b = make_button();
        b.handle_event(&Event::MousePress {
            pos: crate::core::Point::new(20, 30),
            button: crate::event::mouse_button::PRIMARY,
            modifiers: 0,
        });
        assert!(b.is_pressed());

        b.handle_event(&Event::MouseMove { pos: crate::core::Point::new(900, 900) });
        assert!(!b.is_pressed(), "dragging off must clear pressed");

        b.handle_event(&Event::MouseMove { pos: crate::core::Point::new(25, 35) });
        assert!(b.is_pressed(), "dragging back must restore pressed");
    }

    #[test]
    fn a_secondary_press_does_not_activate() {
        // The secondary button opens a context menu; it must not also fire the action.
        let mut b = make_button();
        b.handle_event(&Event::MousePress {
            pos: crate::core::Point::new(20, 30),
            button: crate::event::mouse_button::SECONDARY,
            modifiers: 0,
        });
        assert!(!b.is_pressed());
    }

    #[test]
    fn losing_focus_mid_press_abandons_the_gesture() {
        let mut b = make_button();
        let canceled = Arc::new(AtomicBool::new(false));
        b.canceled.connect({
            let flag = Arc::clone(&canceled);
            move || flag.store(true, Ordering::SeqCst)
        });

        b.handle_event(&Event::MousePress {
            pos: crate::core::Point::new(20, 30),
            button: crate::event::mouse_button::PRIMARY,
            modifiers: 0,
        });
        b.handle_event(&Event::FocusLost);
        assert!(!b.is_pressed());
        assert!(canceled.load(Ordering::SeqCst));
    }

    // ── Visual focus (P0-5) ────────────────────────────────────────────

    #[test]
    fn a_pointer_click_focuses_without_drawing_a_focus_ring() {
        let mut b = make_button();
        b.handle_event(&Event::FocusGained { reason: FocusReason::Pointer });
        assert!(b.is_focused(), "the control is the keyboard target");
        assert!(!b.visual_focus(), "but the user is on the pointer, so no ring is drawn");
    }

    #[test]
    fn tab_and_shortcut_focus_draw_the_focus_ring() {
        for reason in [FocusReason::Tab, FocusReason::BackTab, FocusReason::Shortcut] {
            let mut b = make_button();
            b.handle_event(&Event::FocusGained { reason });
            assert!(b.is_focused());
            assert!(b.visual_focus(), "{reason:?} must draw the ring");
        }
    }

    /// The Material metrics profile keeps the inset stroke exactly as it was.
    ///
    /// Focus-ring styling is now selected by the theme's metrics profile, so this is the
    /// regression guard that the default profile did not acquire the macOS halo. The inner
    /// ring is a 2 px stroke and no box-blur filter is emitted.
    #[cfg(device_profile)]
    #[test]
    fn a_material_focused_button_keeps_the_inset_ring() {
        let _theme_guard = crate::style::theme_test_guard();
        assert!(crate::theme::global_theme_manager().set_theme("default"));

        let mut b = make_button();
        b.handle_event(&Event::FocusGained { reason: FocusReason::Tab });
        let svg = crate::widget::svg::render_to_svg(&mut b);
        assert!(
            svg.contains(&format!("stroke-width=\"{FOCUS_RING_WIDTH}\"")),
            "the Material profile must keep the inset ring stroke: {svg}"
        );
        assert!(
            !svg.contains("shadow_blur_"),
            "the Material profile must not acquire the macOS halo: {svg}"
        );
    }

    /// A theme on the macOS metrics profile draws the focus ring outside the control.
    ///
    /// The macOS focus indicator is a blurred accent glow around the control, not an inset
    /// stroke. The halo goes through the renderer's existing box-blur command, so the marker
    /// to assert is the emitted blur filter (and the absence of the 2 px inset stroke).
    #[cfg(device_profile)]
    #[test]
    fn a_macos_focused_button_draws_an_outer_halo() {
        let _theme_guard = crate::style::theme_test_guard();
        let mut theme = crate::theme::Theme::default();
        theme.name = "p2-1-macos".to_string();
        theme.metrics_profile = ControlMetricsProfile::Macos;
        {
            let mut manager = crate::theme::global_theme_manager();
            manager.register_theme(theme);
            assert!(manager.set_theme("p2-1-macos"));
        }

        let mut b = make_button();
        b.handle_event(&Event::FocusGained { reason: FocusReason::Tab });
        let svg = crate::widget::svg::render_to_svg(&mut b);
        assert!(
            svg.contains(&format!("shadow_blur_{FOCUS_HALO_BLUR_RADIUS}")),
            "the macOS profile must draw the halo through the box-blur primitive: {svg}"
        );
        assert!(
            !svg.lines().any(|line| line.contains("<rect") && line.contains("stroke-width=\"2\"")),
            "the halo replaces the inset stroke rather than joining it: {svg}"
        );

        assert!(crate::theme::global_theme_manager().set_theme("default"));
    }

    /// The Material metrics profile is the existing constant table, applied to the control.
    ///
    /// This is the regression guard for the default appearance: the button's hint is the old
    /// `64x40` floor and the painted face keeps the old 4 px radius.
    #[cfg(device_profile)]
    #[test]
    fn the_material_profile_keeps_the_button_constants() {
        let _theme_guard = crate::style::theme_test_guard();
        assert!(crate::theme::global_theme_manager().set_theme("default"));

        let mut b = Button::new("Go".into(), Rect::new(0, 0, 200, 40));
        crate::theme::apply_active_theme(&mut b);
        let hint = b.implicit_size();
        let svg = crate::widget::svg::render_to_svg(&mut b);

        assert_eq!(hint.width, dimensions::BUTTON_MIN.width, "the old width floor: {hint:?}");
        assert_eq!(hint.height, dimensions::BUTTON_MIN.height, "the old height floor: {hint:?}");
        assert!(
            svg.contains("rx=\"4\""),
            "Material keeps the old 4 px radius rather than acquiring a profile shape: {svg}"
        );
    }

    /// A Macos-profile theme draws the shorter, rounder macOS push button.
    ///
    /// The floor is `64x28`, and the corner radius follows the profile's shape token rather
    /// than the theme's generic border radius. The height assertion is the load-bearing one:
    /// the profile's floor must also own the vertical air, or Material's 8 px padding would
    /// push the content-plus-padding term past 28 and the profile would be invisible.
    #[cfg(device_profile)]
    #[test]
    fn a_macos_profile_theme_shortens_and_rounds_the_button() {
        let _theme_guard = crate::style::theme_test_guard();
        let mut theme = crate::theme::Theme::default();
        theme.name = "p2-6-button-macos".to_string();
        theme.metrics_profile = ControlMetricsProfile::Macos;
        {
            let mut manager = crate::theme::global_theme_manager();
            manager.register_theme(theme);
            assert!(manager.set_theme("p2-6-button-macos"));
        }

        let mut b = make_button();
        crate::theme::apply_active_theme(&mut b);
        let hint = b.implicit_size();
        let svg = crate::widget::svg::render_to_svg(&mut b);
        assert!(crate::theme::global_theme_manager().set_theme("default"));

        assert_eq!(hint.height, 28, "macOS's push button is 28 px tall: {hint:?}");
        assert_eq!(
            hint.width,
            dimensions::BUTTON_MIN.width,
            "the profile keeps Material's width floor: {hint:?}"
        );
        assert!(
            svg.contains("rx=\"6\""),
            "the profile's 6 px radius reaches the painted face: {svg}"
        );
    }

    /// The Cupertino profile keeps the Material button row unchanged.
    #[cfg(device_profile)]
    #[test]
    fn the_cupertino_profile_keeps_the_button_constants() {
        let _theme_guard = crate::style::theme_test_guard();
        let mut theme = crate::theme::Theme::default();
        theme.name = "p2-6-button-cupertino".to_string();
        theme.metrics_profile = ControlMetricsProfile::Cupertino;
        {
            let mut manager = crate::theme::global_theme_manager();
            manager.register_theme(theme);
            assert!(manager.set_theme("p2-6-button-cupertino"));
        }

        let mut b = Button::new("Go".into(), Rect::new(0, 0, 200, 40));
        crate::theme::apply_active_theme(&mut b);
        let hint = b.implicit_size();
        let svg = crate::widget::svg::render_to_svg(&mut b);
        assert!(crate::theme::global_theme_manager().set_theme("default"));

        assert_eq!(hint.width, dimensions::BUTTON_MIN.width, "Cupertino is the Material row");
        assert_eq!(hint.height, dimensions::BUTTON_MIN.height);
        assert!(svg.contains("rx=\"4\""), "and the Material radius: {svg}");
    }

    /// A **default** button is drawn differently, and a normal one is untouched.
    ///
    /// # The defect this pins
    ///
    /// `default_button` was stored, published in the schema and toggled by the constructor while
    /// `draw` never read it, so `set_default(true)` redrew the same square. On desktop the default
    /// button is the one Enter activates, and every convention says so on screen -- without it the
    /// user cannot tell which button the key will press. The sibling `focus_ring` tests could not
    /// cover this: a default button need not be focused, and a focused one need not be default.
    ///
    /// Both halves are asserted, and the second is the load-bearing one: a document that merely
    /// *differs* is satisfied by any extra ink, so the emphasis is pinned to a **heavier stroke on
    /// the ring's own inset** -- the same rectangle the focus ring uses, which is what keeps the two
    /// from overlapping when a default button is also focused.
    #[test]
    fn a_default_button_is_drawn_with_an_emphasis_ring() {
        let _theme_guard = crate::style::theme_test_guard();
        let mut plain = make_button();
        let plain_svg = crate::widget::svg::render_to_svg(&mut plain);

        let mut default = make_button();
        assert!(!default.is_default(), "a button is not the default unless it says so");
        default.set_default(true);
        assert!(default.is_default(), "and it reports what was set");
        let default_svg = crate::widget::svg::render_to_svg(&mut default);

        assert_ne!(
            plain_svg, default_svg,
            "a default button must not paint the same as an ordinary one"
        );
        assert!(
            default_svg.contains(&format!("stroke-width=\"{DEFAULT_EMPHASIS_WIDTH}\"")),
            "the emphasis is a stroke of its own width, not a shifted border: {default_svg}"
        );
        assert!(
            !plain_svg.contains(&format!("stroke-width=\"{DEFAULT_EMPHASIS_WIDTH}\"")),
            "an ordinary button has no such stroke: {plain_svg}"
        );
    }

    /// A declared `background_gradient` must actually be painted.
    ///
    /// Pins the defect: the field was settable, mergeable and CSS-expressible, but no painter read
    /// it, so a button with a gradient drew a solid fill. This asserts the rendered SVG grows a
    /// gradient definition that the solid button does not have.
    #[test]
    fn a_declared_background_gradient_is_painted() {
        use crate::style::Gradient;
        let _theme_guard = crate::style::theme_test_guard();

        let mut plain = make_button();
        let plain_svg = crate::widget::svg::render_to_svg(&mut plain);

        let mut gradient_button = make_button();
        let gradient = Gradient::linear(Point::new(0, 0), Point::new(0, 40))
            .add_stop(0.0, Color::rgb(255, 0, 0))
            .add_stop(1.0, Color::rgb(0, 0, 255));
        gradient_button.set_style(gradient_button.style().clone().with_gradient(gradient));
        let gradient_svg = crate::widget::svg::render_to_svg(&mut gradient_button);

        assert_ne!(
            plain_svg, gradient_svg,
            "a gradient must change what is painted, not be silently dropped"
        );
        assert!(
            gradient_svg.contains("linearGradient"),
            "the gradient must reach the SVG as a paint server: {gradient_svg}"
        );
        assert!(
            !plain_svg.contains("linearGradient"),
            "a button with no gradient must not paint one: {plain_svg}"
        );
    }

    #[test]
    fn a_keyboard_activation_keeps_the_ring() {
        let mut b = make_button();
        let clicks = Arc::new(core::sync::atomic::AtomicUsize::new(0));
        b.base.clicked.connect({
            let counter = Arc::clone(&clicks);
            move || {
                counter.fetch_add(1, Ordering::SeqCst);
            }
        });
        b.handle_event(&Event::FocusGained { reason: FocusReason::Shortcut });
        b.handle_event(&Event::KeyPress { key: 32, modifiers: 0 });
        assert_eq!(clicks.load(Ordering::SeqCst), 1);
        assert!(b.visual_focus());
    }

    #[test]
    fn a_disabled_button_never_arms_and_never_clicks() {
        let mut b = make_button();
        let clicked = Arc::new(AtomicBool::new(false));
        b.base.clicked.connect({
            let flag = Arc::clone(&clicked);
            move || flag.store(true, Ordering::SeqCst)
        });
        b.set_enabled_state(false);
        b.handle_event(&Event::MousePress {
            pos: crate::core::Point::new(20, 30),
            button: crate::event::mouse_button::PRIMARY,
            modifiers: 0,
        });
        assert!(!b.is_pressed());
        b.handle_event(&Event::MouseRelease {
            pos: crate::core::Point::new(20, 30),
            button: crate::event::mouse_button::PRIMARY,
        });
        assert!(!clicked.load(Ordering::SeqCst));
    }

    #[cfg(feature = "touch")]
    #[test]
    fn event_touch_begin_presses() {
        let mut b = make_button();
        let event = Event::TouchBegin { pos: Point::new(15, 25), touch_id: 0 };
        b.handle_event(&event);
        assert!(b.is_pressed());
        assert_eq!(b.state(), ButtonState::Pressed);
    }

    #[cfg(feature = "touch")]
    #[test]
    fn event_touch_end_releases_and_clicks() {
        let mut b = make_button();
        let clicked = Arc::new(AtomicBool::new(false));
        b.base.clicked.connect({
            let flag = Arc::clone(&clicked);
            move || {
                flag.store(true, Ordering::SeqCst);
            }
        });

        b.press();
        let event = Event::TouchEnd { pos: Point::new(15, 25), touch_id: 0 };
        b.handle_event(&event);
        assert!(!b.is_pressed());
        assert_eq!(b.state(), ButtonState::Normal);
        assert!(clicked.load(Ordering::SeqCst));
    }

    #[cfg(feature = "touch")]
    #[test]
    fn event_tap_triggers_click() {
        let mut b = make_button();
        let clicked = Arc::new(AtomicBool::new(false));
        b.base.clicked.connect({
            let flag = Arc::clone(&clicked);
            move || {
                flag.store(true, Ordering::SeqCst);
            }
        });

        let event = Event::Tap { pos: Point::new(15, 25) };
        b.handle_event(&event);
        assert!(clicked.load(Ordering::SeqCst));
        // Tap does not change pressed state
        assert!(!b.is_pressed());
    }

    /// D09-EVT-02: a touch cancel resets the latch without firing `clicked`.
    ///
    /// A platform `TouchCancel` is normalised into a `TouchEnd`, which the button treated as a
    /// completed gesture and clicked. The internal cancel event must instead behave like an
    /// abandoned gesture: the pressed latch is cleared and `canceled` (not `clicked`) fires.
    #[cfg(feature = "touch")]
    #[test]
    fn event_touch_cancel_resets_without_clicking() {
        let mut b = make_button();
        let clicked = Arc::new(AtomicBool::new(false));
        let canceled = Arc::new(AtomicBool::new(false));
        b.base.clicked.connect({
            let flag = Arc::clone(&clicked);
            move || {
                flag.store(true, Ordering::SeqCst);
            }
        });
        b.canceled.connect({
            let flag = Arc::clone(&canceled);
            move || {
                flag.store(true, Ordering::SeqCst);
            }
        });

        b.press();
        assert!(b.is_pressed());
        let cancel = crate::event::translator::touch_cancel(Point::new(15, 25), 0);
        b.handle_event(&cancel);

        assert!(!b.is_pressed(), "a cancelled gesture must clear the latch");
        assert!(!b.is_grabbed(), "a cancelled gesture must drop the grab");
        assert!(!clicked.load(Ordering::SeqCst), "a cancel must not click");
        assert!(canceled.load(Ordering::SeqCst), "a cancel reports the abandoned gesture");
    }

    /// D09-EVT-02: a cancel when nothing is pressed is a harmless no-op (no phantom click).
    #[cfg(feature = "touch")]
    #[test]
    fn event_touch_cancel_without_a_press_is_a_noop() {
        let mut b = make_button();
        let clicked = Arc::new(AtomicBool::new(false));
        b.base.clicked.connect({
            let flag = Arc::clone(&clicked);
            move || {
                flag.store(true, Ordering::SeqCst);
            }
        });
        let cancel = crate::event::translator::touch_cancel(Point::new(15, 25), 0);
        b.handle_event(&cancel);
        assert!(!b.is_pressed());
        assert!(!clicked.load(Ordering::SeqCst));
    }

    #[test]
    fn event_disabled_ignores_mouse_down() {
        let mut b = make_button();
        b.set_enabled_state(false);
        let event = Event::MouseDown((Point::new(15, 25), 0));
        b.handle_event(&event);
        assert!(!b.is_pressed());
        assert_eq!(b.state(), ButtonState::Disabled);
    }

    #[test]
    fn event_disabled_ignores_mouse_up() {
        let mut b = make_button();
        b.set_enabled_state(false);
        // Even if somehow pressed, disabled handle_event should not process MouseUp
        let event = Event::MouseUp((Point::new(15, 25), 0));
        b.handle_event(&event);
        assert!(!b.is_pressed());
    }

    #[cfg(feature = "touch")]
    #[test]
    fn event_disabled_ignores_tap() {
        let mut b = make_button();
        b.set_enabled_state(false);
        let clicked = Arc::new(AtomicBool::new(false));
        b.base.clicked.connect({
            let flag = Arc::clone(&clicked);
            move || {
                flag.store(true, Ordering::SeqCst);
            }
        });
        let event = Event::Tap { pos: Point::new(15, 25) };
        b.handle_event(&event);
        assert!(!clicked.load(Ordering::SeqCst));
    }

    #[cfg(feature = "touch")]
    #[test]
    fn event_disabled_ignores_touch_begin() {
        let mut b = make_button();
        b.set_enabled_state(false);
        let event = Event::TouchBegin { pos: Point::new(15, 25), touch_id: 0 };
        b.handle_event(&event);
        assert!(!b.is_pressed());
    }

    #[cfg(feature = "touch")]
    #[test]
    fn event_disabled_ignores_touch_end() {
        let mut b = make_button();
        b.set_enabled_state(false);
        let event = Event::TouchEnd { pos: Point::new(15, 25), touch_id: 0 };
        b.handle_event(&event);
        assert!(!b.is_pressed());
    }

    // ── 8. Widget trait delegation ─────────────────────────────────────
    #[test]
    fn widget_trait_id_and_kind() {
        let b = make_button();
        assert!(b.id() != 0);
        assert_eq!(b.kind(), WidgetKind::Button);
    }

    #[test]
    fn widget_trait_geometry() {
        let mut b = make_button();
        assert_eq!(b.geometry(), rect());
        let new_rect = Rect::new(0, 0, 200, 50);
        b.set_geometry(new_rect);
        assert_eq!(b.geometry(), new_rect);
    }

    #[test]
    fn widget_trait_visibility() {
        let mut b = make_button();
        assert!(b.is_visible());
        b.hide();
        assert!(!b.is_visible());
        b.show();
        assert!(b.is_visible());
    }

    #[test]
    fn widget_trait_enabled() {
        let mut b = make_button();
        assert!(b.is_enabled());
        b.set_enabled(false);
        assert!(!b.is_enabled());
        assert_eq!(b.state(), ButtonState::Disabled);
        b.set_enabled(true);
        assert!(b.is_enabled());
        assert_eq!(b.state(), ButtonState::Normal);
    }

    #[test]
    fn widget_trait_parent_and_children() {
        let mut b = make_button();
        assert!(b.parent().is_none());
        assert!(b.children().is_empty());

        let child: crate::core::ObjectId = 42;
        let parent: crate::core::ObjectId = 99;

        b.set_parent(Some(parent));
        assert_eq!(b.parent(), Some(parent));

        b.add_child(child);
        assert_eq!(b.children(), &[child]);

        b.remove_child(child);
        assert!(b.children().is_empty());
    }

    #[test]
    fn widget_trait_min_max_size() {
        let mut b = make_button();
        assert!(b.min_size().is_none());
        assert!(b.max_size().is_none());

        b.set_min_size(Some(Size::new(80, 24)));
        assert_eq!(b.min_size(), Some(Size::new(80, 24)));

        b.set_max_size(Some(Size::new(400, 200)));
        assert_eq!(b.max_size(), Some(Size::new(400, 200)));
    }

    #[test]
    fn widget_trait_tooltip() {
        let mut b = make_button();
        assert_eq!(b.tooltip(), "");
        b.set_tooltip("Save".into());
        assert_eq!(b.tooltip(), "Save");
    }

    #[test]
    fn widget_trait_style() {
        let mut b = make_button();
        let default_style = b.style().clone();
        // Verify we can set a modified style via builder pattern.
        let new_style = default_style.clone().with_background(Color::rgb(255, 0, 0));
        b.set_style(new_style.clone());
        assert_eq!(b.style().background_color, new_style.background_color);
    }

    #[test]
    fn widget_trait_signals() {
        let b = make_button();
        // Verify signals are accessible via Widget trait
        let _ = b.hover_signal();
        let _ = b.mouse_down_signal();
        let _ = b.mouse_up_signal();
        let _ = b.key_down_signal();
        let _ = b.key_up_signal();
        let _ = b.focus_gained_signal();
        let _ = b.focus_lost_signal();
        let _ = b.redraw_requested_signal();
        let _ = b.layout_requested_signal();
    }

    #[test]
    fn widget_trait_connection_scope() {
        let b = make_button();
        let _scope = b.connection_scope();
    }

    // ── 11. Visibility toggling ────────────────────────────────────────
    #[test]
    fn test_button_visibility_toggle() {
        let mut b = make_button();
        assert!(b.is_visible());

        // Single hide
        b.hide();
        assert!(!b.is_visible());

        // Double hide is idempotent
        b.hide();
        assert!(!b.is_visible());

        // Show restores
        b.show();
        assert!(b.is_visible());

        // Double show is idempotent
        b.show();
        assert!(b.is_visible());

        // Multiple toggle cycles
        for _ in 0..3 {
            b.hide();
            assert!(!b.is_visible());
            b.show();
            assert!(b.is_visible());
        }
    }

    // ── 12. Animated interaction (BLUE21 P0-4) ─────────────────────────

    /// The transition advances toward the pressed state and then **stops**.
    ///
    /// The boolean return is the load-bearing part: a `tick` that kept reporting `true` after the
    /// value had settled would keep the host scheduling frames forever. The engine existed and
    /// nothing called it, so this is the first control to prove the contract end to end.
    #[test]
    fn the_interaction_transition_settles_and_reports_so() {
        let mut b = make_button();
        b.set_hovered(true);

        // A hover moves the fill, so at least one tick must report more work to do.
        assert!(
            b.tick(30),
            "a hover must start a transition, so the first tick reports another frame"
        );

        // Ticking well past the theme's 200 ms settles it exactly, and then reports completion.
        let mut frames = 1;
        while b.tick(1000) {
            frames += 1;
            assert!(frames < 100, "the transition must terminate rather than tick forever");
        }
        assert_eq!(b.interaction_progress.value(), 0.5, "a hover settles exactly on its target");

        // Settled means settled: **each** further tick reports no work. Asserted in a loop rather
        // than once, because a single `!tick()` would also pass for an implementation that
        // alternated between reporting work and not — the defect this pins is "keeps asking for
        // frames after the value has stopped changing", and only repetition can see it.
        for frame in 0..5 {
            assert!(
                !b.tick(16),
                "a settled transition must report that no frame is needed (tick {frame})"
            );
            assert_eq!(b.interaction_progress.value(), 0.5, "and must not drift while settled");
        }
    }

    /// A state change that arrives mid-flight re-aims the same progress rather than restarting.
    #[test]
    fn a_press_during_a_hover_re_aims_rather_than_restarting() {
        let mut b = make_button();
        b.set_hovered(true);
        b.tick(50);
        let partway = b.interaction_progress.value();
        assert!(partway > 0.0 && partway < 0.5, "the hover is in flight: {partway}");

        b.set_pressed(true);
        assert!(b.tick(1), "the press must continue the movement, not settle on the first frame");
        assert!(
            b.interaction_progress.value() > partway,
            "a press must move the progress onward from where the hover left it, not back to zero"
        );
    }

    /// A disabled control rests, whatever its press flags say.
    #[test]
    fn a_disabled_button_rests_regardless_of_its_flags() {
        let mut b = make_button();
        b.set_enabled(false);
        assert_eq!(b.interaction_target_progress(), 0.0);
        assert!(
            !b.tick(1000),
            "a disabled button has nothing to animate toward, so no frame is owed"
        );
    }

    /// The transition is visible: a fully-progressed hover paints a different fill.
    #[test]
    fn the_progress_changes_what_is_painted() {
        // Holds the crate-wide theme guard: this test renders, and a concurrent
        // test that switches the appearance would otherwise change a later frame.
        let _theme_guard = crate::style::theme_test_guard();
        let rect = Rect::new(0, 0, 120, 32);
        let mut resting = Button::new("Go".to_string(), rect);
        let rest_svg = crate::widget::svg::render_to_svg(&mut resting);

        let mut hovered = Button::new("Go".to_string(), rect);
        hovered.set_hovered(true);
        while hovered.tick(1000) {}
        let hover_svg = crate::widget::svg::render_to_svg(&mut hovered);

        assert_ne!(
            rest_svg, hover_svg,
            "a completed hover transition must change the rendered fill"
        );
    }

    /// A push button's label is centred in its own box, not hung off its left padding.
    ///
    /// A button's `contentItem` is centred and Material M3 centres the `TextButton`
    /// child, so a label flush to the left edge is wrong however wide the button is — and at
    /// the census rectangle the button is 240 px wide while a 14 px "Sample" is 50 px, which
    /// is what made `snapshots/svg/button.svg` read as a left-aligned caption.
    ///
    /// The assertion is on the **ink**, not on an element's attribute: text leaves the backend
    /// as the `font8x8` rectangles the rasteriser fills (see
    /// `SvgPaintBackend::execute_command`), so the document holds a picture of the label rather
    /// than a `<text>` element to read. That is the stronger check — the old form asserted the
    /// `x` the backend had written down, so a label drawn a pixel off its own reported origin
    /// would still have passed.
    #[test]
    fn the_label_is_centred_in_the_button() {
        let rect = Rect::new(0, 0, 240, 40);
        let mut b = Button::new("Sample".to_string(), rect);
        let svg = crate::widget::svg::render_to_svg(&mut b);
        let (x, _, right, _) = crate::widget::svg::text_ink_box(&svg)
            .unwrap_or_else(|| panic!("the button must paint its label: {svg}"));
        assert!(right > x, "the label laid down ink: {x}..{right}");
        let font = Font::default();
        let mut backend = crate::render::SvgPaintBackend::new(Size::new(240, 40));
        let width = RenderContext::new(&mut backend).measure_text("Sample", &font).width as i32;
        let content = ControlMetrics::content_box(
            rect,
            EdgeOffsets {
                left: dimensions::BUTTON_PADDING_H,
                right: dimensions::BUTTON_PADDING_H,
                top: 0,
                bottom: 0,
            },
        );
        // The centred origin is an upper bound on where the ink can begin: a glyph's first
        // bitmap column is set somewhere inside the 8-column raster, so the ink starts at the
        // origin or to its right and never before it.
        let centred = content.x + (content.width as i32 - width) / 2;
        assert!(
            x >= centred && x < centred + width / 4,
            "the label must sit in the middle of the button's padded box: ink at {x}, centre {centred}"
        );
        assert!(
            x > rect.x + dimensions::BUTTON_PADDING_H as i32,
            "not flush to the padding: ink at {x}"
        );
    }

    /// A narrow button still honours the minimum padding rather than touching its border.
    #[test]
    fn a_label_wider_than_the_padding_keeps_the_padding() {
        let rect = Rect::new(0, 0, 90, 40);
        let mut b = Button::new("A very wide label".to_string(), rect);
        let svg = crate::widget::svg::render_to_svg(&mut b);
        let (x, _, right, _) = crate::widget::svg::text_ink_box(&svg)
            .unwrap_or_else(|| panic!("the button must paint its fitted label: {svg}"));
        assert!(right > x, "the label laid down ink: {x}..{right}");
        assert!(
            x >= rect.x + dimensions::BUTTON_PADDING_H as i32 - 1,
            "a fitted label must not start left of the button's own padding, got {x}"
        );
    }

    #[test]
    fn test_button_zero_geometry() {
        // Zero width/height — should not panic
        let _b = Button::new("Zero".into(), Rect::new(0, 0, 0, 0));

        // Negative position — should not panic
        let _b = Button::new("Neg".into(), Rect::new(-10, -20, 100, 30));

        // Zero geometry with negative position — should not panic
        let _b = Button::new("All".into(), Rect::new(-5, -5, 0, 0));
    }

    #[test]
    fn test_button_signal_not_emitted_on_same_value() {
        let mut b = make_button();
        let changed_count = Arc::new(AtomicBool::new(false));
        let c = Arc::clone(&changed_count);
        b.base.changed.connect(move || {
            c.store(true, Ordering::SeqCst);
        });

        // set_text does NOT emit base.changed — it only calls request_redraw()
        b.set_text("Click");
        assert!(
            !changed_count.load(Ordering::SeqCst),
            "set_text with same value should not emit changed"
        );

        // Even setting different text does not emit base.changed
        b.set_text("Different");
        assert!(
            !changed_count.load(Ordering::SeqCst),
            "set_text with different value should not emit changed"
        );
    }

    // ── §B.7 template: `button_with_icon` ─────────────────────────────

    /// A button with an icon is `icon + gap + label` on one line, and its own hint grows when
    /// either part grows.
    ///
    /// This is §B.9's mechanical test in its smallest form (§B.7 chose this template first for
    /// exactly that reason): the room a control claims must be derived from its children's own
    /// sizes, so a part that gets wider pushes the rest along rather than overlapping it.
    #[cfg(device_profile)]
    #[test]
    fn the_implicit_size_is_the_icon_and_the_label_on_one_line() {
        // The assertions are the Material floor, so a concurrent test that activates the
        // Macos profile must not change what this control reports.
        let _theme_guard = crate::style::theme_test_guard();
        assert!(crate::theme::global_theme_manager().set_theme("default"));

        let empty = Button::new(String::new(), Rect::new(0, 0, 200, 40));
        let short = Button::new("Go".into(), Rect::new(0, 0, 200, 40));
        let long = Button::new("Go somewhere much further".into(), Rect::new(0, 0, 200, 40));

        assert!(
            long.implicit_size().width > short.implicit_size().width,
            "a longer label must claim more room"
        );
        // The floor is the load-bearing term: a two-character label must still be pressable.
        assert_eq!(
            short.implicit_size().width,
            dimensions::BUTTON_MIN.width,
            "a short label is below the floor, so the floor is the answer"
        );
        assert_eq!(short.implicit_size().height, dimensions::BUTTON_MIN.height);
        // A label-less button is not a special case: it is content plus padding, floored.
        assert!(empty.implicit_size().width >= dimensions::BUTTON_MIN.width);
    }

    /// An icon reserves a leading slot, and the label follows it.
    ///
    /// The two boxes are read from one derivation each (`icon_rect` / `label_rect`), and they tile
    /// with exactly the shared gap between them. Before this, the icon's advance was a term added
    /// to a hand-rolled label estimate and the two were never compared, so a wider icon could not
    /// push the label — the drift §B.9 names.
    #[test]
    fn the_icon_is_leading_and_the_label_follows_it() {
        let plain = Button::new("Save".into(), Rect::new(0, 0, 200, 40));
        // Without an icon the label slot is the whole content box, and there is no icon box.
        assert!(plain.icon_rect().is_none(), "a button with no icon has no icon box");
        let box_ = ControlMetrics::content_box(
            plain.geometry(),
            EdgeOffsets::symmetric(dimensions::BUTTON_PADDING_V, dimensions::BUTTON_PADDING_H),
        );
        assert_eq!(plain.label_rect(), box_, "the label takes the whole content box alone");

        // The exact arithmetic, stated in the open: the content width is the measured label plus
        // both paddings, and the result is *floored* — so below the floor the floor is the answer
        // and above it the arithmetic is. `"Save"` is four clusters and measures below the floor,
        // which is exactly the case the floor exists for.
        let measured =
            estimate_text_width("Save", &Font::default(), 1.0) + 2 * dimensions::BUTTON_PADDING_H;
        assert!(measured < dimensions::BUTTON_MIN.width, "this label must be below the floor");
        assert_eq!(
            plain.implicit_size().width,
            dimensions::BUTTON_MIN.width,
            "below the floor the floor is the answer, not the raw arithmetic"
        );

        // Above the floor the same arithmetic is what is returned, which is the other half of
        // `implicit_size`'s `max`.
        let long = Button::new("A considerably longer label".into(), Rect::new(0, 0, 200, 40));
        let long_measured = estimate_text_width(long.text(), &Font::default(), 1.0)
            + 2 * dimensions::BUTTON_PADDING_H;
        assert!(long_measured > dimensions::BUTTON_MIN.width);
        assert_eq!(long.implicit_size().width, long_measured);
    }

    /// The size hint and the drawn box agree about the button's height.
    ///
    /// `draw` derives its painted height from `size_hint`, so this is the property that keeps a
    /// wide button from being drawn as a wide pill: the reported height is what it paints.
    #[test]
    fn the_hint_height_is_the_height_painted() {
        // `implicit_size` reads the active metrics profile, which is theme state; hold the
        // crate-wide guard so a neighbouring appearance switch cannot change the answer.
        let _guard = crate::style::theme_test_guard();
        let b = make_button();
        let hint = b.implicit_size();
        // A census cell is far taller than a button; the drawn box must be the hint, not the cell.
        let tall = Button::new("OK".into(), Rect::new(0, 0, 240, 120));
        assert_eq!(tall.implicit_size().height, hint.height);
        assert!(tall.implicit_size().height < 120, "the hint must not follow the cell height");
    }

    /// BLUE24 §10A.6 criterion 2: the two styles differ in **geometry**, not only in colour.
    ///
    /// # Why "geometry" and not "a colour changed"
    ///
    /// The whole claim of the surface channel is that a flat theme and a dimensional one are two
    /// corners of one parameter space — so the observable difference must be a *shape*: a bevelled
    /// face draws two light/shade lines along its boundary, and a flat one draws none. Asserting on
    /// a fill colour would pass for any theme change, including one that never reached the bevel.
    ///
    /// The bevel is drawn with `<line>` elements, which is what the assertion counts — the same
    /// marker `check_control_feature_visible_in_own_snapshot` uses for `group_box`.
    ///
    /// # Why this is gated on `device_profile`
    ///
    /// It drives the real theme manager, which exists only where a device profile does. On
    /// `mini`/`embedded` there is no theme to resolve, so there is nothing for the channel to
    /// carry — the subject of the test does not exist there, which is what a gate has to say.
    #[cfg(device_profile)]
    #[test]
    fn flat_and_dimensional_styles_differ_in_geometry() {
        use crate::theme::{global_theme_manager, Theme};

        let _theme_guard = crate::style::theme_test_guard();

        // The global manager is what `apply_active_theme` reads, so the test drives the same
        // path production does: register both presets, draw under one, then the other.
        {
            let mut manager = global_theme_manager();
            manager.register_theme(Theme::default());
            manager.register_theme(Theme::dimensional());
        }

        let draw_under = |name: &str| {
            assert!(global_theme_manager().set_theme(name), "{name} is registered");
            let mut b = make_button();
            crate::theme::apply_theme_to_widget(&mut b);
            crate::widget::svg::render_to_svg(&mut b)
        };

        let flat = draw_under("default");
        let dimensional = draw_under("dimensional");

        let bevel_lines = |svg: &str| svg.matches("<line").count();
        assert_eq!(
            bevel_lines(&flat),
            0,
            "the flat style must draw no bevel at all, or the default snapshots would change"
        );
        assert!(
            bevel_lines(&dimensional) > 0,
            "and the dimensional style must draw the bevel's light/shade pair, or the channel \
             never reached the control"
        );

        // Restore the light default so a later test in this binary starts where it expects.
        assert!(global_theme_manager().set_theme("default"));
    }

    // ── D09-STYLE-02: the hint measures with the font the paint uses ──

    /// A caller-authored `style.font` must move the hint together with the painted label.
    ///
    /// # The defect this pins
    ///
    /// `implicit_size()` measured with `Font::default()` while `draw()` painted with
    /// `style.font`, so a custom face or a scaled theme font left the hint at the default size:
    /// the drawn label was `draw_text_fitted` into a box that was too small and got squeezed or
    /// truncated. Both paths now read `metrics::effective_font`.
    #[test]
    fn a_custom_font_grows_the_button_hint() {
        let mut baseline = Button::new("A wide label".into(), Rect::new(0, 0, 240, 40));
        let base_hint = baseline.implicit_size();

        let mut big = Button::new("A wide label".into(), Rect::new(0, 0, 240, 40));
        big.set_style(crate::style::WidgetStyle::default().with_font(Font::simple("Test", 30.0)));
        let big_hint = big.implicit_size();

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
        // The draw path reads the same font, so its ink must be wider too.
        let base_ink = ink_width(&mut baseline);
        let big_ink = ink_width(&mut big);
        assert!(big_ink > base_ink, "the painted label must widen too: {big_ink} vs {base_ink}");
    }

    /// A text scale above 1.0, resolved through the theme, must widen the hint.
    #[cfg(device_profile)]
    #[test]
    fn a_theme_text_scale_above_one_grows_the_button_hint() {
        use crate::style::environment::{
            install_environment, uninstall_environment, EnvironmentProvider,
        };
        use crate::style::MotionPreference;
        use crate::theme::{global_theme_manager, AppearanceMode, Theme};

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
            let mut manager = global_theme_manager();
            manager.register_theme(Theme::default());
            manager.set_appearance(AppearanceMode::Light);
        }
        let mut at_one = Button::new("Scaled label".into(), Rect::new(0, 0, 240, 40));
        crate::theme::apply_active_theme(&mut at_one);
        let hint_one = at_one.implicit_size();

        install_environment(Box::new(Scaled(1.5)));
        let mut scaled = Button::new("Scaled label".into(), Rect::new(0, 0, 240, 40));
        crate::theme::apply_active_theme(&mut scaled);
        let hint_large = scaled.implicit_size();

        let _ = uninstall_environment();
        if let Some(previous) = previous {
            install_environment(previous);
        }

        assert!(
            hint_large.width > hint_one.width,
            "a text scale above 1.0 must widen the button hint: {} vs {}",
            hint_large.width,
            hint_one.width
        );
    }

    /// The ink width of the label a button paints, read back from an SVG render.
    fn ink_width(button: &mut Button) -> i32 {
        let svg = crate::widget::svg::render_to_svg(button);
        crate::widget::svg::text_ink_box(&svg).map(|(x, _, right, _)| right - x).unwrap_or(0)
    }

    /// A hover frame submits damage bounded by the painted box and the margin.
    #[cfg(not(alloc_frugal))]
    #[test]
    fn a_hover_frame_submits_the_painted_box_with_the_damage_margin() {
        use crate::widget::runtime;

        let id = runtime::register(crate::compat::Box::new(Button::new(
            "Go".to_string(),
            Rect::new(4, 8, 80, 32),
        )))
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
        use crate::platform::RecordingInvalidations;
        use crate::widget::runtime;

        let id = runtime::register(crate::compat::Box::new(Button::new(
            "Go".to_string(),
            Rect::new(4, 8, 80, 32),
        )))
        .expect("registry");
        assert_eq!(runtime::repaint_mode(id), runtime::RepaintMode::Full);

        // A recorder that accepts narrowed repaints proves the control did not offer one:
        // the `Full` arm falls back to the whole-control invalidation.
        let log: &'static RecordingInvalidations =
            Box::leak(Box::new(RecordingInvalidations::new()));
        let dispatched = crate::platform::with_recorded_invalidations(log, || {
            runtime::dispatch_event(id, &Event::mouse_enter(1, 1))
        });
        assert!(dispatched);
        assert_eq!(log.calls(), vec![(id, None)], "Full mode keeps the whole-control repaint");
        assert!(
            runtime::dirty_rects(id).is_empty(),
            "Full mode records no damage, so the control repaints whole"
        );
        runtime::unregister(id);
    }

    // ── Icon size, shortcut, flat, auto-default (R001, R002, R006) ─────

    #[test]
    fn a_larger_icon_size_grows_the_hint_and_the_icon_box() {
        let mut button = make_button();
        button.set_icon_source(Some(IconSource::Builtin(crate::widget::IconName::Info)));
        let small = button.implicit_size();
        let icon = button.icon_rect().expect("an icon is present");
        assert_eq!(icon.width, dimensions::BUTTON_ICON_SIZE);

        button.set_icon_size(20);
        assert_eq!(button.icon_size(), 20);
        assert!(button.implicit_size().width > small.width, "the reserved advance follows");
        assert_eq!(button.icon_rect().expect("icon").width, 20);
    }

    #[test]
    fn the_shortcut_property_round_trips_and_parses() {
        let mut button = make_button();
        assert_eq!(button.get("shortcut"), Ok(CapabilityValue::Null));
        button
            .set("shortcut", CapabilityValue::String("Primary+S".into()))
            .expect("a parseable shortcut is writable");
        assert_eq!(
            button.get("shortcut"),
            Ok(CapabilityValue::String("Primary+S".into())),
            "the canonical spelling round-trips"
        );
        assert_eq!(
            button.set("shortcut", CapabilityValue::String("Primary+S+T".into())),
            Err(CapabilityAccessError::TypeMismatch),
            "a malformed chord is refused rather than half-parsed"
        );
        button.set("shortcut", CapabilityValue::Null).expect("null clears it");
        assert!(button.shortcut().is_none());
    }

    #[test]
    fn the_declared_shortcut_activates_the_button() {
        use crate::shortcut::{Key, Shortcut};
        let mut button = make_button();
        button.set_shortcut(Some(Shortcut::primary(Key::S)));
        let clicked = Arc::new(AtomicBool::new(false));
        let flag = clicked.clone();
        button.clicked_signal().connect(move || flag.store(true, Ordering::SeqCst));

        let modifiers = 0b1000;
        button.handle_event(&Event::KeyPress { key: 83, modifiers });
        assert!(clicked.load(Ordering::SeqCst), "the declared chord activates the button");

        // A different chord is a different binding.
        clicked.store(false, Ordering::SeqCst);
        button.handle_event(&Event::KeyPress { key: 83, modifiers: 0 });
        assert!(!clicked.load(Ordering::SeqCst), "the bare key is not the declared chord");
    }

    #[test]
    fn flat_and_auto_flags_are_published_and_writable() {
        let mut button = make_button();
        for name in ["flat", "auto_default", "auto_exclusive", "drag_enabled", "accepts_drops"] {
            assert_eq!(button.get(name), Ok(CapabilityValue::Bool(false)), "{name} defaults off");
            button.set(name, CapabilityValue::Bool(true)).expect("writable");
            assert_eq!(button.get(name), Ok(CapabilityValue::Bool(true)));
        }
        assert!(button.is_flat());
    }

    #[test]
    fn auto_default_follows_focus() {
        let mut button = make_button();
        button.set_auto_default(true);
        button.handle_event(&Event::FocusGained { reason: FocusReason::Tab });
        assert!(button.is_default(), "focus makes an auto-default button the default");
        button.handle_event(&Event::FocusLost);
        assert!(!button.is_default(), "losing focus gives the emphasis back");
    }

    #[test]
    fn auto_exclusive_buttons_move_the_default_emphasis_inside_one_parent() {
        let mut first = make_button();
        let mut second = Button::new("Other".into(), Rect::new(10, 60, 120, 36));
        first.set_parent(Some(42));
        second.set_parent(Some(42));
        first.set_auto_exclusive(true);
        second.set_auto_exclusive(true);

        first.click();
        assert!(first.is_default(), "the clicked member carries the emphasis");
        second.click();
        assert!(!first.is_default(), "the group has one emphasised member");
        assert!(second.is_default());
    }

    #[test]
    fn the_elide_property_round_trips() {
        let mut button = make_button();
        assert_eq!(button.get("elide"), Ok(CapabilityValue::String("right".into())));
        button.set("elide", CapabilityValue::String("middle".into())).expect("writable");
        assert_eq!(button.elide_mode(), TextElideMode::ElideMiddle);
        assert_eq!(
            button.set("elide", CapabilityValue::String("sideways".into())),
            Err(CapabilityAccessError::TypeMismatch)
        );
    }

    // ── Menu attachment (R001, R003) ───────────────────────────────────

    #[cfg(full_widgets)]
    #[test]
    fn an_attached_menu_opens_and_reports_the_activated_row() {
        use crate::widget::menu_toolbar::menu_button::MenuItem;
        use std::sync::Mutex;

        let mut button = Button::new("File".into(), Rect::new(10, 20, 120, 36));
        button.set_menu(vec![MenuItem::new(1, "Open"), MenuItem::new(2, "Save")]);
        assert_eq!(button.menu_items().len(), 2);

        let triggered = Arc::new(Mutex::new(0u64));
        let sink = triggered.clone();
        button.item_triggered.connect(move |id| {
            *sink.lock().unwrap() = *id;
        });

        button.open_menu();
        assert!(button.is_menu_open());
        let panel = button.menu_rect();
        let first_row = crate::widget::menu_toolbar::menu_button::menu_item_rect(panel, 0);
        button.handle_event(&Event::MousePress {
            pos: Point::new(first_row.x + 4, first_row.y + 4),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(*triggered.lock().unwrap(), 1);
        assert!(!button.is_menu_open(), "activating a row closes the menu");
    }

    #[cfg(full_widgets)]
    #[test]
    fn the_menu_owns_the_pointer_while_it_is_open() {
        use crate::widget::menu_toolbar::menu_button::MenuItem;

        let mut button = Button::new("File".into(), Rect::new(10, 20, 120, 36));
        button.set_menu(vec![MenuItem::new(1, "Open")]);
        button.open_menu();
        let panel = button.menu_rect();
        assert!(
            button.contains_point(Point::new(panel.x + 2, panel.y + 2)),
            "a press on the popup must route to the button that owns it"
        );

        button.handle_event(&Event::KeyPress { key: key_codes::ESCAPE, modifiers: 0 });
        assert!(!button.is_menu_open(), "Escape dismisses the attached menu");
    }

    #[cfg(full_widgets)]
    #[test]
    fn a_context_menu_request_anchors_the_attached_menu_at_the_pointer() {
        use crate::widget::menu_toolbar::menu_button::MenuItem;

        let mut button = Button::new("File".into(), Rect::new(0, 0, 100, 30));
        assert!(
            !Widget::context_menu_event(&mut button, Point::new(5, 5)),
            "a button without a menu has nothing to show"
        );
        button.set_menu(vec![MenuItem::new(1, "Open")]);
        assert!(Widget::context_menu_event(&mut button, Point::new(40, 50)));
        assert!(button.is_menu_open());
        assert!(button.menu_rect().contains_point(Point::new(41, 51)));
    }

    // ── Drag and drop (R003) ───────────────────────────────────────────

    #[test]
    fn a_drag_enabled_button_offers_its_caption() {
        use crate::event::dnd::DragSource;

        let mut button = make_button();
        assert!(
            DragSource::drag_payload(&button, Point::new(20, 30)).is_none(),
            "dragging is opt-in"
        );
        button.set_drag_enabled(true);
        let payload = DragSource::drag_payload(&button, Point::new(20, 30))
            .expect("a draggable button carries its caption");
        assert!(payload.is_type("text/plain"));
        assert_eq!(payload.label, "Click");
        assert_eq!(payload.item_id, "Click");
    }

    #[test]
    fn a_drop_accepting_button_takes_the_dropped_caption() {
        use crate::event::dnd::{DragPayload, DropEffect, DropTarget};

        let mut button = make_button();
        let payload = DragPayload::new("text/plain", "ignored").with_label("Dropped");
        assert!(button.drop_target_mut().is_none(), "accepting drops is opt-in");
        button.set_accepts_drops(true);
        assert!(button.drop_target_mut().is_some());
        assert!(DropTarget::can_accept(&button, &payload));
        assert!(!DropTarget::can_accept(&button, &DragPayload::new("image/png", "x")));
        assert_eq!(
            DropTarget::on_drop(&mut button, &payload, Point::new(10, 20)),
            DropEffect::Copy,
            "text is copied, not consumed"
        );
        assert_eq!(button.text(), "Dropped");
    }
}
