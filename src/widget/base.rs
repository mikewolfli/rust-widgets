// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Base widget state — shared struct used by all concrete controls.

use super::WidgetKind;
use crate::core::{ObjectId, Point, Rect, Size};
use crate::event::{Event, EventHandler, FocusReason};
use crate::object::Object;
use crate::signal::{ConnectionScope, GenericSignal, Signal1};
use crate::style::WidgetStyle;

/// Shared widget state and signals used by concrete controls.
///
/// Every widget in the crate embeds one of these and exposes it through
/// [`crate::widget::Widget::base`] / `base_mut`, so this is the set of
/// capabilities every control inherits:
///
/// * identity — [`Self::id`] and [`Self::kind`]
/// * geometry — [`Self::geometry`] plus optional [`Self::min_size`] and
///   [`Self::max_size`] hints, which the widget itself never enforces
/// * a parent/child tree of [`ObjectId`]s — a *declaration* of nesting, not
///   ownership; nothing here keeps the referenced widgets alive and nothing
///   updates the links when a widget is destroyed
/// * visibility and enablement flags, which this type only stores and reports
///   — [`Self::handle_event`] does not consult them
/// * tooltip text, DPI scale factor, and a [`WidgetStyle`]
/// * the standard signals listed below, emitted by [`Self::handle_event`]
///
/// Construct with [`Self::new`]. The only invariants this type keeps are that the
/// DPI scale is at least `0.1`, that an id is unique per instance, and that
/// [`Self::remove_child`] removes all occurrences of the given id; everything
/// else is plain state the owner is responsible for keeping consistent.
pub struct BaseWidget {
    pub(crate) object: Object,
    pub(crate) kind: WidgetKind,
    pub(crate) geometry: Rect,
    pub(crate) min_size: Option<Size>,
    pub(crate) max_size: Option<Size>,
    pub(crate) parent: Option<ObjectId>,
    /// Child widget IDs. Under mini, this is a fixed-capacity `MiniVec<ObjectId, 64>`
    /// (compile-time known size, no heap alloc). Under desktop, `alloc::vec::Vec`.
    pub(crate) children: crate::compat::MiniVec<ObjectId>,
    /// How many additions were refused because `children` was at its fixed capacity.
    ///
    /// Always zero on desktop builds, where the list is unbounded. On the mini profile it is the
    /// only trace of a child that could not be stored: the failing `push` result used to be
    /// discarded outright.
    pub(crate) child_overflow_count: usize,
    pub(crate) visible: bool,
    pub(crate) enabled: bool,
    pub(crate) mouse_pressed: bool,
    pub(crate) tooltip: crate::compat::MiniString,
    pub(crate) dpi_scale: f32,
    pub(crate) style: WidgetStyle,
    pub(crate) connection_scope: ConnectionScope,
    /// Emitted when a click-like interaction is received.
    pub clicked: GenericSignal,
    /// Emitted when hover/move interaction is observed.
    pub hover: Signal1<Point>,
    /// Emitted when mouse/pointer button is pressed.
    pub mouse_down: Signal1<(Point, u32)>,
    /// Emitted when mouse/pointer button is released.
    pub mouse_up: Signal1<(Point, u32)>,
    /// Emitted when keyboard key is pressed.
    pub key_down: Signal1<(u32, u32)>,
    /// Emitted when keyboard key is released.
    pub key_up: Signal1<(u32, u32)>,
    /// Emitted when focus-like state is gained.
    pub focus_gained: GenericSignal,
    /// Emitted when focus-like state is lost.
    pub focus_lost: GenericSignal,
    /// Emitted when redraw is requested.
    pub redraw_requested: GenericSignal,
    /// Emitted when layout is requested.
    pub layout_requested: GenericSignal,
    /// Emitted when a stateful value changes (e.g., slider value, checkbox state).
    pub changed: GenericSignal,
    /// Whether [`Self::request_redraw`] has ever been called on this widget.
    ///
    /// Set once and never cleared, because it answers a question about the control's
    /// whole life rather than about the current frame: "does this control ever ask to be
    /// A surface that never asks keeps `RepaintMode::Full` and pays no
    /// bookkeeping, which is the trade-off `widget::runtime::should_track_damage`
    /// makes.
    ///
    /// A `Cell` rather than a plain `bool` because [`Self::request_redraw`] takes
    /// `&self`: the flag has to be written from a shared borrow, and the alternative —
    /// threading `&mut self` through ~1000 call sites — is exactly the churn the single
    /// chokepoint exists to avoid. Only this one bit is interior-mutable; a write is a
    /// non-atomic store that cannot tear, so no data race is introduced.
    ///
    /// Read back by `widget::runtime::should_track_damage` — a plain code span rather
    /// than a link because that module is not compiled on `mini`, and a doc link that
    /// resolves only on some profiles fails the doc build on the others.
    pub(crate) ever_requested_redraw: core::cell::Cell<bool>,
    /// Whether the pointer is currently over this widget.
    ///
    /// # Why this lives here and not in each control
    ///
    /// The runtime already decides *which* control the pointer is over —
    /// `widget::runtime::dispatch_hover_transition` synthesises exactly one
    /// `MouseEnter`/`MouseLeave` pair as the pointer crosses a boundary — so "am I
    /// hovered?" is a fact the base class can record once. Controls that each
    /// re-derive it from `MouseMove` coordinates (as `Button` used to) get a
    /// different answer from the one the runtime already committed to, and the rest
    /// never answer at all. Recording it here is what makes `widget_state()` able to
    /// report `Hover` for a control that overrides nothing.
    pub(crate) hovered: bool,
    /// Whether a *gesture* is live on this widget, and the widget should paint
    /// pressed.
    ///
    /// Distinct from [`Self::grabbed`]: this one answers "should I look held", and a
    /// pointer that drags off the control clears it while the grab stays. A control
    /// that tracked only this bit could not tell "the pointer wandered off" from "the
    /// gesture ended", which is why the pair exists (see [`Self::grabbed`]).
    pub(crate) pressed: bool,
    /// Whether this widget currently owns the pointer gesture.
    ///
    /// Set on a press that resolves to this widget and cleared on release or ungrab.
    /// A widget that owns the grab keeps receiving move/release events even after the
    /// pointer leaves its rectangle, which is what lets a drag start inside and finish
    /// outside — and is exactly why `pressed` and `grabbed` cannot be one flag.
    pub(crate) grabbed: bool,
    /// Why this widget has keyboard focus, or `None` when it does not.
    ///
    /// The reason is stored, not just the fact, because it answers a *second* question
    /// at paint time: whether to draw the focus ring. A pointer press focuses a control
    /// without a ring; `Tab` and shortcuts focus it with one
    /// ([`FocusReason::draws_focus_ring`]). Storing the reason is therefore the one
    /// derivation both [`crate::widget::Widget::widget_state`] and a draw path need.
    pub(crate) focus_reason: Option<FocusReason>,
}
impl BaseWidget {
    /// Create base widget state and core signals.
    ///
    /// Starts visible and enabled, with no parent or children, an empty tooltip,
    /// DPI scale `1.0` (i.e. unscaled pixels), a default [`WidgetStyle`], and all
    /// signals unconnected. The widget's [`ObjectId`] is freshly allocated, as is
    /// the object name `class_name`, which is used for debugging and object
    /// lookup rather than for behaviour.
    pub fn new(kind: WidgetKind, geometry: Rect, class_name: &'static str) -> Self {
        Self {
            object: Object::new(class_name),
            kind,
            geometry,
            min_size: None,
            max_size: None,
            parent: None,
            children: crate::compat::MiniVec::new(),
            child_overflow_count: 0,
            visible: true,
            enabled: true,
            mouse_pressed: false,
            tooltip: crate::compat::MiniString::new(),
            dpi_scale: 1.0,
            style: WidgetStyle::default(),
            connection_scope: ConnectionScope::new(),
            clicked: GenericSignal::new(),
            hover: Signal1::new(),
            mouse_down: Signal1::new(),
            mouse_up: Signal1::new(),
            key_down: Signal1::new(),
            key_up: Signal1::new(),
            focus_gained: GenericSignal::new(),
            focus_lost: GenericSignal::new(),
            redraw_requested: GenericSignal::new(),
            layout_requested: GenericSignal::new(),
            changed: GenericSignal::new(),
            ever_requested_redraw: core::cell::Cell::new(false),
            hovered: false,
            pressed: false,
            grabbed: false,
            focus_reason: None,
        }
    }
    // -- Base accessors --
    /// The widget's unique identifier, shared with its backing [`Object`].
    pub fn id(&self) -> ObjectId {
        self.object.id()
    }
    /// The kind of control this widget is; fixed at construction.
    pub fn kind(&self) -> WidgetKind {
        self.kind
    }
    /// The widget's rectangle in parent-relative coordinates, in logical pixels.
    ///
    /// This is the raw stored value: `min_size`/`max_size` are hints the widget
    /// does not apply to it, and no clipping against the parent is performed.
    pub fn geometry(&self) -> Rect {
        self.geometry
    }
    /// Replaces the widget's rectangle.
    ///
    /// Purely stores the value — no clamping against `min_size`/`max_size`, and
    /// no redraw or layout is requested, so callers that need one must ask for it
    /// via [`Self::request_redraw`].
    pub fn set_geometry(&mut self, geometry: Rect) {
        self.geometry = geometry;
    }
    /// The minimum size hint, or `None` when unset. Advisory only.
    pub fn min_size(&self) -> Option<Size> {
        self.min_size
    }
    /// The maximum size hint, or `None` when unset. Advisory only.
    pub fn max_size(&self) -> Option<Size> {
        self.max_size
    }
    /// Sets the minimum size hint; `None` clears it. Not enforced by this type,
    /// and not validated against the maximum.
    pub fn set_min_size(&mut self, min_size: Option<Size>) {
        self.min_size = min_size;
    }
    /// Sets the maximum size hint; `None` clears it. Not enforced here, and not
    /// validated against the minimum.
    pub fn set_max_size(&mut self, max_size: Option<Size>) {
        self.max_size = max_size;
    }
    /// The parent's id, or `None` for a root widget. Stored as-is.
    pub fn parent(&self) -> Option<ObjectId> {
        self.parent
    }
    /// Sets the parent id; `None` detaches the widget.
    ///
    /// This does *not* update the old or new parent's child list, so the two
    /// directions must be kept in sync by the caller.
    pub fn set_parent(&mut self, parent: Option<ObjectId>) {
        self.parent = parent;
    }
    /// The ids of this widget's children, in insertion order.
    ///
    /// The list can contain ids of widgets that no longer exist; this type never
    /// validates or reclaims entries.
    pub fn children(&self) -> &[ObjectId] {
        &self.children
    }
    /// Appends `child` to the child list, without touching the child's own parent
    /// link.
    ///
    /// Duplicates are *not* rejected, so adding the same id twice requires two
    /// removals to undo.
    ///
    /// # Capacity
    ///
    /// On desktop builds the list grows without bound and this always succeeds. Under the
    /// `alloc_frugal` (mini) profile the list is a fixed-capacity `heapless::Vec<_, 64>`: an
    /// add past 64 is refused. Use [`BaseWidget::try_add_child`] when the caller must know, or
    /// [`BaseWidget::child_capacity`]/[`BaseWidget::child_overflow_count`] to observe it
    /// afterwards.
    pub fn add_child(&mut self, child: ObjectId) {
        let _ = self.try_add_child(child);
    }

    /// Appends `child`, reporting whether it was stored.
    ///
    /// Returns `true` on desktop builds (unbounded) and on the mini profile while the list has
    /// room. Returns `false` when the mini profile's fixed capacity is reached, so a caller can
    /// react instead of losing the child. [`BaseWidget::child_overflow_count`] also records it,
    /// so the loss is observable even through [`BaseWidget::add_child`].
    pub fn try_add_child(&mut self, child: ObjectId) -> bool {
        #[cfg(alloc_frugal)]
        {
            // `heapless::Vec::push` fails atomically once the buffer is full. Counting the
            // refusal is what keeps this from being the silent drop it used to be: nothing
            // else in the widget layer was watching the discarded `Result`.
            match self.children.push(child) {
                Ok(()) => true,
                Err(_) => {
                    self.child_overflow_count = self.child_overflow_count.saturating_add(1);
                    false
                }
            }
        }
        #[cfg(not(alloc_frugal))]
        {
            self.children.push(child);
            true
        }
    }

    /// The maximum number of children this widget can hold, or `None` when unbounded.
    ///
    /// `None` on desktop builds, where the child list is a growable `Vec`. On the mini profile
    /// it is the fixed capacity, so a caller building a large container can check before adding
    /// rather than discovering the loss afterwards.
    pub fn child_capacity(&self) -> Option<usize> {
        #[cfg(alloc_frugal)]
        {
            Some(crate::compat::MINI_VEC_CAPACITY)
        }
        #[cfg(not(alloc_frugal))]
        {
            None
        }
    }

    /// How many additions this widget refused because it was at capacity.
    ///
    /// Non-zero only on the mini profile, and only after [`BaseWidget::add_child`] or
    /// [`BaseWidget::try_add_child`] hit the cap. The counter exists because the previous
    /// behaviour discarded the failing `push` result outright: a container that dropped half its
    /// children looked identical to one that was never given them.
    pub fn child_overflow_count(&self) -> usize {
        self.child_overflow_count
    }
    /// Removes every occurrence of `child` from the child list, keeping the
    /// relative order of the rest.
    ///
    /// Only edits this list: the removed child's parent link is left pointing
    /// back here. Does nothing if `child` is not present.
    pub fn remove_child(&mut self, child: ObjectId) {
        self.children.retain(|&id| id != child);
    }
    /// Makes the widget visible by setting its visibility flag.
    ///
    /// Does not request a redraw, and does not touch the visibility of any child.
    pub fn show(&mut self) {
        self.visible = true;
    }
    /// Hides the widget by clearing its visibility flag.
    ///
    /// Does not request a redraw.
    pub fn hide(&mut self) {
        self.visible = false;
    }
    /// Whether the visibility flag is set. True unless [`Self::hide`] was called.
    pub fn is_visible(&self) -> bool {
        self.visible
    }
    /// Sets the enabled flag, which controls whether the widget accepts input.
    ///
    /// This type only stores the flag; concrete widgets are expected to check
    /// [`Self::is_enabled`] in their own event handling, as `MessageBox` does.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
    /// Whether the widget is enabled. True unless [`Self::set_enabled`] cleared it.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// A foreground colour that honours the enabled flag.
    ///
    /// # Why this exists
    ///
    /// `set_enabled(false)` only stores a flag, so a widget that gates its input but
    /// paints with a fixed colour ends up in the worst possible state: it silently
    /// swallows clicks while looking fully interactive. Roughly a third of the paint
    /// paths in this crate had that shape, each with its own ad-hoc grey.
    ///
    /// Callers pick their enabled colour and let this substitute the shared disabled
    /// one, so "is this disabled?" is answered by one constant instead of a
    /// per-widget literal:
    ///
    /// ```text
    /// let color = self.base.effective_foreground(Color::FOREGROUND);
    /// ```
    ///
    /// Widgets whose disabled appearance needs more than a colour swap (a dimmed
    /// track, a suppressed hover highlight) should still branch on
    /// [`Self::is_enabled`] directly — this covers the common case.
    pub fn effective_foreground(&self, enabled_color: crate::core::Color) -> crate::core::Color {
        if self.enabled {
            enabled_color
        } else {
            crate::core::Color::DISABLED_FOREGROUND
        }
    }

    /// A fill colour that honours the enabled flag.
    ///
    /// The counterpart to [`Self::effective_foreground`] for backgrounds: a disabled
    /// surface is desaturated toward the shared disabled tint rather than left at its
    /// full-strength colour, so a disabled panel does not read as an active one.
    ///
    /// Implemented as a channel-wise mix toward `DISABLED_FOREGROUND` rather than a
    /// flat replacement, which keeps each widget's own hue recognisable while making
    /// the reduced emphasis visible.
    pub fn effective_fill(&self, enabled_color: crate::core::Color) -> crate::core::Color {
        if self.enabled {
            enabled_color
        } else {
            crate::core::Color::disabled_variant_of(enabled_color)
        }
    }
    /// Replaces the tooltip text with an already-localised string.
    pub fn set_tooltip(&mut self, tooltip: crate::compat::MiniString) {
        self.tooltip = tooltip;
    }
    /// The current tooltip text; an empty string when none was set.
    pub fn tooltip(&self) -> &str {
        &self.tooltip
    }
    /// The device-pixel-ratio scale factor, never below `0.1`. Defaults to `1.0`.
    pub fn dpi_scale(&self) -> f32 {
        self.dpi_scale
    }
    /// Sets the DPI scale factor, clamping values below `0.1` up to `0.1` so a
    /// degenerate scale cannot reach layout maths. `NaN` is *not* handled: NaN
    /// comparisons are false, so a NaN argument is stored as-is.
    pub fn set_dpi_scale(&mut self, scale: f32) {
        self.dpi_scale = scale.max(0.1);
    }
    /// Sets the tooltip by looking up a translation `key`.
    ///
    /// The key is resolved through the i18n catalogue whenever that capability is
    /// compiled in; without it `crate::translate_key` returns the key verbatim, which
    /// is convenient for debugging but must not be relied on to deliver real
    /// translations.
    ///
    /// # Why this does not name `crate::i18n` directly
    ///
    /// `i18n` is a **capability** feature, and the module only exists when it is on, so
    /// naming it here forces a `cfg`. An earlier revision used
    /// `#[cfg(feature = "desktop")]`, which is the *wrong* question: `tablet` and
    /// `mobile` enable `i18n` without `desktop` (see `Cargo.toml`, and
    /// `src/platform/profile.rs`'s note that "`feature = \"i18n\"` is a capability
    /// feature, not a profile gate"). Those profiles therefore compiled the
    /// untranslated branch into a build that had a working, initialised catalogue, and a
    /// tooltip read `common.button.cancel` instead of `Cancel`.
    ///
    /// `crate::translate_key` is the capability-correct entry point that keeps the
    /// `cfg` in one place.
    pub fn set_translated_tooltip(&mut self, key: &str) {
        self.tooltip = crate::compat::mini_string_from(crate::translate_key(key));
    }
    /// Borrows the widget's visual style (colours, padding, touch target, etc.).
    pub fn style(&self) -> &WidgetStyle {
        &self.style
    }
    /// Mutably borrows the widget's visual style, for in-place adjustment.
    ///
    /// A change made through this reference does not by itself trigger a redraw.
    pub fn style_mut(&mut self) -> &mut WidgetStyle {
        &mut self.style
    }
    /// Check if a point is within this widget's geometry, optionally expanded
    /// to meet the minimum touch target size set in `WidgetStyle.touch_target`.
    ///
    /// When `touch_target` is set and the widget's visual geometry is smaller
    /// than that target, the effective hit-test area is expanded outward while
    /// keeping the visual center unchanged.
    pub fn contains_point_with_touch_expansion(&self, point: Point) -> bool {
        let rect = match self.style.touch_target {
            Some(min_size) => self.geometry.expand_to_touch_target(min_size),
            None => self.geometry,
        };
        rect.contains_point(point)
    }
    /// Replaces the entire style with `style`.
    ///
    /// Unlike [`Self::style_mut`] this overwrites every field, discarding any
    /// previous customisations. No redraw is requested.
    pub fn set_style(&mut self, style: WidgetStyle) {
        self.style = style;
    }
    /// The scope that owns this widget's signal connections; dropping it
    /// disconnects them.
    ///
    /// Used to tie connection lifetime to the widget, so a widget that outlives
    /// its handlers does not keep them alive.
    pub fn connection_scope(&self) -> &ConnectionScope {
        &self.connection_scope
    }
    /// The hover signal, emitted with the pointer position. Same signal as the
    /// public `hover` field, exposed by reference.
    pub fn hover_signal(&self) -> &Signal1<Point> {
        &self.hover
    }
    /// The pointer-down signal, emitted with the position and the raw button
    /// number. Same signal as the public `mouse_down` field.
    pub fn mouse_down_signal(&self) -> &Signal1<(Point, u32)> {
        &self.mouse_down
    }
    /// The pointer-up signal, emitted with the position and the raw button
    /// number. Same signal as the public `mouse_up` field.
    pub fn mouse_up_signal(&self) -> &Signal1<(Point, u32)> {
        &self.mouse_up
    }
    /// The key-press signal, emitted with the platform key code and the modifier
    /// bitmask. Same signal as the public `key_down` field.
    pub fn key_down_signal(&self) -> &Signal1<(u32, u32)> {
        &self.key_down
    }
    /// The key-release signal, emitted with the platform key code and the modifier
    /// bitmask. Same signal as the public `key_up` field.
    pub fn key_up_signal(&self) -> &Signal1<(u32, u32)> {
        &self.key_up
    }
    /// The focus-gained signal. Same signal as the public `focus_gained` field.
    pub fn focus_gained_signal(&self) -> &GenericSignal {
        &self.focus_gained
    }
    /// The focus-lost signal. Same signal as the public `focus_lost` field.
    pub fn focus_lost_signal(&self) -> &GenericSignal {
        &self.focus_lost
    }
    /// The redraw-requested signal, emitted by [`Self::request_redraw`].
    pub fn redraw_requested_signal(&self) -> &GenericSignal {
        &self.redraw_requested
    }
    /// The layout-requested signal, emitted by [`Self::request_layout`].
    pub fn layout_requested_signal(&self) -> &GenericSignal {
        &self.layout_requested
    }
    /// Whether a pointer button is currently held on this widget.
    ///
    /// Tracked only if something calls [`Self::set_mouse_pressed`]; the default
    /// routing in [`Self::handle_event`] does not maintain it.
    pub fn is_mouse_pressed(&self) -> bool {
        self.mouse_pressed
    }
    /// Records whether a pointer button is held on this widget.
    ///
    /// Pure bookkeeping; it does not emit `mouse_down`/`mouse_up` nor request a
    /// redraw.
    pub fn set_mouse_pressed(&mut self, pressed: bool) {
        self.mouse_pressed = pressed;
    }

    // -- Interaction state -------------------------------------------------------
    //
    // The four facts below are maintained by `handle_event` for *every* widget, so a
    // control that overrides nothing still knows whether it is hovered, pressed or
    // focused. `Widget::widget_state` turns them into the single state a theme is
    // looked up by; draw paths that want the raw facts read them directly.

    /// Whether the pointer is currently over this widget.
    ///
    /// Set by the `MouseEnter` arm and cleared by `MouseLeave`, both synthesised by
    /// `widget::runtime::dispatch_hover_transition`. A host that has no hover concept
    /// can still state the fact through [`Self::set_hovered`].
    pub fn is_hovered(&self) -> bool {
        self.hovered
    }
    /// Sets the hovered flag without requesting a redraw.
    ///
    /// Prefer this over synthesising a `MouseEnter`/`MouseLeave` event when the caller
    /// is asserting a state rather than reporting input (a designer preview, a touch
    /// backend with no hover). `handle_event` uses the same field, so both spellings
    /// of the fact stay in one place.
    pub fn set_hovered(&mut self, hovered: bool) {
        self.hovered = hovered;
    }
    /// Whether a live gesture says this widget should paint pressed.
    ///
    /// Continuous, not an edge: dragging off the control clears it and dragging back
    /// restores it (the `MouseMove` arm re-resolves it against the pointer position).
    pub fn is_pressed(&self) -> bool {
        self.pressed
    }
    /// Records whether the widget should paint pressed. See [`Self::pressed`] for why
    /// this is separate from [`Self::grabbed`].
    pub fn set_pressed(&mut self, pressed: bool) {
        self.pressed = pressed;
    }
    /// Whether this widget owns the pointer gesture.
    pub fn is_grabbed(&self) -> bool {
        self.grabbed
    }
    /// Takes or releases the pointer grab.
    ///
    /// The grab is the *ownership* half of a gesture and outlives `pressed`: a drag
    /// that wanders off the control keeps it so the release can still be routed here.
    pub fn set_grabbed(&mut self, grabbed: bool) {
        self.grabbed = grabbed;
    }
    /// Why this widget currently holds keyboard focus, or `None` when it does not.
    ///
    /// The *presence* of focus is [`Self::focus_reason`] being `Some`; there is no
    /// separate boolean, because the two could then disagree. Use
    /// [`Self::draws_focus_ring`] rather than matching on the reason at a draw site.
    pub fn focus_reason(&self) -> Option<FocusReason> {
        self.focus_reason
    }
    /// Whether a focus ring should be painted right now.
    ///
    /// One derivation for every control: a widget that is focused for a reason that
    /// does not warrant a ring (a pointer press) must not paint one.
    pub fn draws_focus_ring(&self) -> bool {
        self.focus_reason.is_some_and(FocusReason::draws_focus_ring)
    }
    /// Records the focus reason directly, without requesting a redraw.
    pub fn set_focus_reason(&mut self, reason: Option<FocusReason>) {
        self.focus_reason = reason;
    }
    /// Asks the host to repaint this widget, and records the damage.
    ///
    /// Takes `&self`, so it can be called from shared references. If nothing is
    /// connected to the signal the request is simply dropped — this does not queue
    /// a redraw by itself.
    ///
    /// # Why this also records damage
    ///
    /// This is the single point every appearance change converges on: the crate has
    /// ~1000 call sites, covering a control mutating its own state, an event handler
    /// reacting to input, and a programmatic property write. Recording the damage
    /// *here* rather than at each mutation site is what makes partial repaint correct
    /// by construction — there is no path that changes what a control looks like
    /// without coming through this function.
    ///
    /// The record is a no-op unless the widget has opted into damage tracking (see
    /// `RepaintMode` in `widget::runtime`), so a caller that never
    /// asked pays one thread-local lookup and nothing else. It is also a no-op for a
    /// widget that is not registered, which is the case for the transient controls
    /// tests build.
    ///
    /// Gated with the runtime: damage tracking lives in `widget::runtime`, which `mini`
    /// does not compile (it is `alloc_frugal` and paints whole frames by design). The
    /// call is skipped there rather than stubbed, because there is no tracker to record
    /// into and the signal below is what `mini` actually uses.
    pub fn request_redraw(&self) {
        // Record the request before anything else can observe it: an observer of the
        // signal below may re-enter the registry, and the whole point of the flag is to
        // answer "has this control ever asked?". The write is one non-atomic store, so
        // it is safe from a shared borrow.
        self.ever_requested_redraw.set(true);
        #[cfg(not(alloc_frugal))]
        crate::widget::runtime::mark_widget_damage(self.id(), self.geometry());
        self.redraw_requested.emit();
    }
    /// Whether [`Self::request_redraw`] has ever been called on this widget.
    ///
    /// Read by `widget::runtime::should_track_damage` to tell a control that
    /// repaints itself from one that never will.
    pub fn has_ever_requested_redraw(&self) -> bool {
        self.ever_requested_redraw.get()
    }
    /// Asks the host to re-run layout for this widget by emitting the layout
    /// signal.
    ///
    /// Like [`Self::request_redraw`], it is only a request: with no listener the
    /// emit has no effect.
    pub fn request_layout(&self) {
        self.layout_requested.emit();
    }
}
/// Default event routing for every widget: forward each recognised input event to
/// the matching typed signal.
///
/// [`Event::MouseMove`] and [`Event::PointerMove`] emit `hover`;
/// `MousePress`/`MouseDown`/`PointerPress` emit `mouse_down`, and their release
/// counterparts emit `mouse_up`; `KeyPress`/`KeyDown` emit `key_down` and
/// `KeyRelease`/`KeyUp` emit `key_up`; `FocusGained`/`FocusLost` emit their
/// signals. Every other event is ignored.
///
/// This routing is unconditional: hit-testing, visibility and the enabled flag are
/// not consulted here, so an implementor that wants those checks must impose them
/// before or after delegating. Note that only bare events are handled —
/// `Event::Click` and `Event::Changed` do *not* emit `clicked` or `changed`, and
/// `mouse_pressed` is not maintained.
impl EventHandler for BaseWidget {
    /// Routes the *primitive* input events to this widget's base signals.
    ///
    /// # What this does and does not emit
    ///
    /// This is deliberately limited to signals whose meaning is fixed for every
    /// widget: pointer movement, press and release, key press and release, and focus
    /// changes. It emits [`BaseWidget::hover`], [`BaseWidget::mouse_down`],
    /// [`BaseWidget::mouse_up`], [`BaseWidget::key_down`], [`BaseWidget::key_up`],
    /// [`BaseWidget::focus_gained`] and [`BaseWidget::focus_lost`].
    ///
    /// It does **not** emit the *semantic* signals — [`BaseWidget::clicked`] or
    /// [`BaseWidget::changed`] — and that is intentional, not an omission. Whether a
    /// click happened depends on the widget: a `Button` needs a press *and* a release
    /// while still armed, a `CheckBox` toggles, a `Slider` changes its value on drag.
    /// Only the widget knows its own gesture, so each one emits these itself (17
    /// widgets emit `clicked`, 12 emit `changed`); see
    /// `base_widgets::button::Button::handle_event` for the pattern.
    ///
    /// Consequently, a widget that never emits `clicked` is not broken — it either
    /// has no click concept (`Label`, `Panel`) or drives a different signal
    /// (`Slider::value_changed`). Callers should read the concrete widget's own
    /// signals rather than expecting the base to supply one.
    fn handle_event(&mut self, event: &Event) {
        // Interaction state first: these four facts are what `Widget::widget_state`
        // reports, and every control inherits them without writing a line. Doing it
        // before the signal fan-out means a slot connected to `mouse_down` already sees
        // `is_pressed()` true, so a handler never has to re-derive the fact itself.
        self.record_interaction_state(event);
        // Default event routing: delegate to typed signals
        match event {
            Event::MouseMove { pos } => {
                self.hover.emit(*pos);
            }
            Event::MousePress { pos, button } => {
                self.mouse_down.emit((*pos, *button));
            }
            Event::MouseRelease { pos, button } => {
                self.mouse_up.emit((*pos, *button));
            }
            Event::MouseDown((pos, button)) => {
                self.mouse_down.emit((*pos, *button));
            }
            Event::MouseUp((pos, button)) => {
                self.mouse_up.emit((*pos, *button));
            }
            Event::PointerMove { pos, .. } => {
                self.hover.emit(*pos);
            }
            Event::PointerPress { pos, button, .. } => {
                self.mouse_down.emit((*pos, *button));
            }
            Event::PointerRelease { pos, button, .. } => {
                self.mouse_up.emit((*pos, *button));
            }
            Event::KeyPress { key, modifiers } => {
                self.key_down.emit((*key, *modifiers));
            }
            Event::KeyRelease { key, modifiers } => {
                self.key_up.emit((*key, *modifiers));
            }
            Event::KeyDown((key, modifiers)) => {
                self.key_down.emit((*key, *modifiers));
            }
            Event::KeyUp((key, modifiers)) => {
                self.key_up.emit((*key, *modifiers));
            }
            Event::FocusGained { .. } => {
                self.focus_gained.emit();
            }
            Event::FocusLost => {
                self.focus_lost.emit();
            }
            _ => { /* Other events are not relevant */ }
        }
    }
}

impl BaseWidget {
    /// Maintains [`Self::hovered`], [`Self::pressed`], [`Self::grabbed`] and
    /// [`Self::focus_reason`] from the primitive input events.
    ///
    /// # Why the base owns this
    ///
    /// These four are true of *every* control, and the events that report them are the
    /// ones the base already routes. A control that re-derived hover from `MouseMove`
    /// coordinates produced a second, possibly different, answer; one that never did
    /// reported `Normal` forever, which is why a `"button:hover"` theme entry was
    /// unreachable. Recording them once here is what makes the state channel real.
    ///
    /// # The move arm re-resolves `pressed` rather than clearing it
    ///
    /// `pressed` is continuous: a press that drags off the control stops painting
    /// pressed and a drag back restores it, but the *grab* survives both so the release
    /// can still be routed here. That is the whole reason the two are separate fields.
    ///
    /// # Why arming here does not silence a control's own signals
    ///
    /// The binary controls emit `pressed`/`released` on the *edge*, and this runs before
    /// their arm. They therefore keep a private signal latch (see `Button`'s
    /// `signaled_pressed`) rather than using this flag as their edge — the flag is the
    /// single source for what is *painted*, the latch for what has been *announced*.
    fn record_interaction_state(&mut self, event: &Event) {
        match event {
            Event::MouseEnter { .. } => self.hovered = true,
            Event::MouseLeave { .. } => self.hovered = false,
            Event::MousePress { pos, button } => {
                // Only a press the runtime actually routed here (or that hit-tests
                // inside) arms the gesture, and never on a disabled control: a disabled
                // control is inert, so its state channel must stay `Disabled` rather than
                // reporting pressed. A themed box that paints outside its rectangle would
                // otherwise look pressed for a click that landed elsewhere.
                if self.enabled
                    && *button == crate::event::mouse_button::PRIMARY
                    && self.contains_point_with_touch_expansion(*pos)
                {
                    self.grabbed = true;
                    self.pressed = true;
                }
            }
            Event::MouseMove { pos } | Event::PointerMove { pos, .. } if self.grabbed => {
                self.pressed = self.contains_point_with_touch_expansion(*pos);
            }
            Event::MouseRelease { .. } | Event::PointerRelease { .. } => {
                self.grabbed = false;
                self.pressed = false;
            }
            Event::FocusGained { reason } => self.focus_reason = Some(*reason),
            Event::FocusLost => self.focus_reason = None,
            _ => { /* Other events carry no base-owned state */ }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::Vec;

    fn make_base() -> BaseWidget {
        BaseWidget::new(WidgetKind::Button, Rect::new(10, 20, 100, 30), "Button")
    }

    #[test]
    fn test_new_creates_widget_with_defaults() {
        let bw = make_base();
        assert_eq!(bw.kind(), WidgetKind::Button);
        assert_eq!(bw.geometry(), Rect::new(10, 20, 100, 30));
        assert!(bw.is_visible());
        assert!(bw.is_enabled());
        assert!(!bw.is_mouse_pressed());
        assert!(bw.tooltip().is_empty());
        assert!((bw.dpi_scale() - 1.0).abs() < 0.01);
    }

    /// The base routes primitive input to its signals.
    ///
    /// Pins the half of the contract that *is* the base's responsibility, so the
    /// documented split between primitive and semantic signals has a test behind it.
    #[test]
    fn base_routes_primitive_input_events_to_its_signals() {
        use std::sync::{Arc, Mutex};

        let mut bw = make_base();
        let hovers = Arc::new(Mutex::new(Vec::new()));
        let hovers_slot = Arc::clone(&hovers);
        bw.hover.connect(move |p| hovers_slot.lock().expect("lock").push(*p));

        let downs = Arc::new(Mutex::new(0usize));
        let downs_slot = Arc::clone(&downs);
        bw.mouse_down.connect(move |_| *downs_slot.lock().expect("lock") += 1);

        bw.handle_event(&Event::MouseMove { pos: Point::new(3, 4) });
        bw.handle_event(&Event::MousePress { pos: Point::new(3, 4), button: 1 });

        assert_eq!(*hovers.lock().expect("lock"), vec![Point::new(3, 4)], "hover is routed");
        assert_eq!(*downs.lock().expect("lock"), 1, "mouse_down is routed");
    }

    /// The base must **not** invent a `clicked` signal.
    ///
    /// Whether a pointer release is a click depends on the widget's own gesture, so
    /// each widget emits it itself (17 do, 12 emit `changed`). If the base ever
    /// started emitting `clicked` unconditionally, every widget would double-emit —
    /// once from here and once from its own handler — so this asserts the absence.
    #[test]
    fn base_does_not_emit_semantic_signals() {
        use std::sync::{Arc, Mutex};

        let mut bw = make_base();
        let clicks = Arc::new(Mutex::new(0usize));
        let clicks_slot = Arc::clone(&clicks);
        bw.clicked.connect(move || *clicks_slot.lock().expect("lock") += 1);

        let changes = Arc::new(Mutex::new(0usize));
        let changes_slot = Arc::clone(&changes);
        bw.changed.connect(move || *changes_slot.lock().expect("lock") += 1);

        // A complete press/release pair, which is what a Button would turn into a
        // click in its own handler.
        bw.handle_event(&Event::MousePress { pos: Point::new(1, 1), button: 1 });
        bw.handle_event(&Event::MouseRelease { pos: Point::new(1, 1), button: 1 });

        assert_eq!(
            *clicks.lock().expect("lock"),
            0,
            "the base must leave `clicked` to the widget: emitting here would \
             double-emit for every control that also emits it"
        );
        assert_eq!(*changes.lock().expect("lock"), 0, "`changed` is likewise the widget's to emit");
    }

    #[test]
    fn test_id_unique_per_instance() {
        let a = make_base();
        let b = make_base();
        assert_ne!(a.id(), b.id());
    }

    #[test]
    fn test_set_geometry_updates() {
        let mut bw = make_base();
        bw.set_geometry(Rect::new(0, 0, 200, 50));
        assert_eq!(bw.geometry(), Rect::new(0, 0, 200, 50));
    }

    #[test]
    fn test_min_max_size() {
        let mut bw = make_base();
        assert!(bw.min_size().is_none());
        assert!(bw.max_size().is_none());

        bw.set_min_size(Some(Size::new(50, 20)));
        bw.set_max_size(Some(Size::new(500, 300)));
        assert_eq!(bw.min_size(), Some(Size::new(50, 20)));
        assert_eq!(bw.max_size(), Some(Size::new(500, 300)));
    }

    #[test]
    fn test_parent_and_children() {
        let mut bw = make_base();
        assert!(bw.parent().is_none());
        assert!(bw.children().is_empty());

        bw.set_parent(Some(100));
        assert_eq!(bw.parent(), Some(100));

        bw.add_child(200);
        bw.add_child(300);
        assert_eq!(bw.children(), &[200, 300]);

        bw.remove_child(200);
        assert_eq!(bw.children(), &[300]);
    }

    #[test]
    fn test_show_hide_visibility() {
        let mut bw = make_base();

        bw.hide();
        assert!(!bw.is_visible());

        bw.show();
        assert!(bw.is_visible());
    }

    #[test]
    fn test_set_enabled() {
        let mut bw = make_base();
        assert!(bw.is_enabled());

        bw.set_enabled(false);
        assert!(!bw.is_enabled());

        bw.set_enabled(true);
        assert!(bw.is_enabled());
    }

    #[test]
    fn test_tooltip() {
        let mut bw = make_base();
        assert!(bw.tooltip().is_empty());

        bw.set_tooltip(crate::compat::into_mini("Help text"));
        assert_eq!(bw.tooltip(), "Help text");
    }

    /// Adding past the mini profile's capacity is reported, not silently dropped.
    ///
    /// `add_child` discarded the failing `push` result, so under `alloc_frugal` a container that
    /// had reached its 64-child cap lost every further child with no trace: `add_child` returns
    /// `()`, nothing logged, and the result was indistinguishable from a container that was never
    /// given them. The refusal is now counted and `try_add_child` returns it.
    #[test]
    fn add_child_reports_capacity_overflow() {
        let mut parent = make_base();
        assert_eq!(parent.child_overflow_count(), 0);

        let cap = parent.child_capacity();
        // Add well past any capacity the profile might have.
        let requested = cap.unwrap_or(0) + 10;
        for id in 1..=requested as u64 {
            parent.add_child(id);
        }

        match cap {
            Some(capacity) => {
                assert_eq!(parent.children().len(), capacity, "the list stops at capacity");
                assert_eq!(
                    parent.child_overflow_count(),
                    requested - capacity,
                    "every refused add must be counted"
                );
                // `try_add_child` reports the refusal directly.
                assert!(!parent.try_add_child(9999), "a full list must refuse");
                // Freeing a slot makes it usable again.
                parent.remove_child(1);
                assert!(parent.try_add_child(9999), "a freed slot must be usable");
            }
            None => {
                assert_eq!(parent.children().len(), requested, "desktop has no cap");
                assert_eq!(parent.child_overflow_count(), 0);
                assert!(parent.try_add_child(9999));
            }
        }
    }

    /// A tooltip longer than the buffer keeps a prefix rather than vanishing.
    ///
    /// `into_mini` used to return an empty string for anything over the capacity, so an
    /// over-long tooltip became indistinguishable from no tooltip at all. On desktop
    /// `MiniString` is unbounded, so the truncation half only applies to `alloc_frugal`.
    #[test]
    fn an_over_long_tooltip_keeps_a_prefix() {
        let mut bw = make_base();
        let long = "y".repeat(crate::compat::MINI_STRING_CAPACITY * 3);
        bw.set_tooltip(crate::compat::into_mini(&long));

        #[cfg(alloc_frugal)]
        assert_eq!(
            bw.tooltip().len(),
            crate::compat::MINI_STRING_CAPACITY,
            "an over-long tooltip must keep a full-capacity prefix"
        );
        #[cfg(not(alloc_frugal))]
        assert_eq!(bw.tooltip().len(), long.len(), "desktop keeps the whole string");

        assert!(!bw.tooltip().is_empty(), "an over-long tooltip must never become empty");
    }

    /// `set_translated_tooltip` must store the **translation**, not the key.
    ///
    /// Gated on `i18n` because it asserts catalogue contents; the no-`i18n` behaviour
    /// (key verbatim, plus a warning) is pinned by the test below, which runs in every
    /// profile.
    ///
    /// The `desktop`-gated revision compiled a key-verbatim fallback into every
    /// non-`desktop` build, including `tablet`/`mobile`, which enable the `i18n`
    /// capability and therefore have a working catalogue and an initialised one. The
    /// symptom was a tooltip reading `common.button.cancel` on a tablet build.
    #[cfg(feature = "i18n")]
    #[test]
    fn test_translated_tooltip_stores_the_translation_not_the_key() {
        crate::i18n::init();
        let mut bw = make_base();
        bw.set_translated_tooltip("common.button.cancel");
        assert_eq!(bw.tooltip(), "Cancel");
        assert_ne!(
            bw.tooltip(),
            "common.button.cancel",
            "the catalogue key must not be what the user ends up reading"
        );
    }

    /// An unknown key still yields something displayable rather than an empty tooltip.
    #[test]
    fn test_translated_tooltip_falls_back_to_the_key_for_unknown_keys() {
        let mut bw = make_base();
        bw.set_translated_tooltip("no.such.key.exists");
        assert_eq!(bw.tooltip(), "no.such.key.exists");
    }

    #[test]
    fn test_dpi_scale_clamps_to_minimum() {
        let mut bw = make_base();

        bw.set_dpi_scale(2.0);
        assert!((bw.dpi_scale() - 2.0).abs() < 0.01);

        bw.set_dpi_scale(0.0); // should clamp to 0.1
        assert!((bw.dpi_scale() - 0.1).abs() < 0.01);
    }

    #[test]
    fn test_contains_point_without_touch_expansion() {
        let bw = make_base(); // Rect(10, 20, 100, 30)
        assert!(bw.contains_point_with_touch_expansion(Point::new(15, 25)));
        assert!(!bw.contains_point_with_touch_expansion(Point::new(0, 0)));
    }

    #[test]
    fn test_contains_point_with_touch_expansion() {
        let mut bw = make_base(); // Rect(10, 20, 100, 30)
        bw.style.touch_target = Some(Size::new(150, 50));

        // Inside expanded area but outside original rect
        assert!(bw.contains_point_with_touch_expansion(Point::new(5, 15)));
        assert!(bw.contains_point_with_touch_expansion(Point::new(115, 55)));

        // Far outside
        assert!(!bw.contains_point_with_touch_expansion(Point::new(-50, -50)));
    }

    #[test]
    fn test_mouse_pressed_state() {
        let mut bw = make_base();
        assert!(!bw.is_mouse_pressed());

        bw.set_mouse_pressed(true);
        assert!(bw.is_mouse_pressed());

        bw.set_mouse_pressed(false);
        assert!(!bw.is_mouse_pressed());
    }

    #[test]
    fn test_style_accessors() {
        let mut bw = make_base();
        let _style = bw.style();
        let _style_mut = bw.style_mut();
        // Just verify methods exist and don't panic
        bw.set_style(WidgetStyle::default());
    }

    #[test]
    fn test_signal_accessors_return_signals() {
        let bw = make_base();
        // All signal accessors should return Some (signal exists)
        let _ = bw.hover_signal();
        let _ = bw.mouse_down_signal();
        let _ = bw.mouse_up_signal();
        let _ = bw.key_down_signal();
        let _ = bw.key_up_signal();
        let _ = bw.focus_gained_signal();
        let _ = bw.focus_lost_signal();
        let _ = bw.redraw_requested_signal();
        let _ = bw.layout_requested_signal();
    }

    #[test]
    fn test_event_routing_mouse_move_emits_hover() {
        let mut bw = make_base();
        let hovered = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let last_pos = std::sync::Arc::new(std::sync::Mutex::new(Point::new(0, 0)));
        let h = hovered.clone();
        let lp = last_pos.clone();
        bw.hover.connect(move |p| {
            h.store(true, std::sync::atomic::Ordering::SeqCst);
            *lp.lock().unwrap() = *p;
        });

        bw.handle_event(&Event::MouseMove { pos: Point::new(50, 25) });
        assert!(hovered.load(std::sync::atomic::Ordering::SeqCst));
        assert_eq!(*last_pos.lock().unwrap(), Point::new(50, 25));
    }

    #[test]
    fn test_event_routing_key_down_emits_signal() {
        let mut bw = make_base();
        let emitted = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let emitted_clone = emitted.clone();
        bw.key_down.connect(move |args| {
            let (key, _mods) = *args;
            if key == 65 {
                emitted_clone.store(true, std::sync::atomic::Ordering::SeqCst);
            }
        });

        bw.handle_event(&Event::KeyDown((65, 0)));
        assert!(emitted.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn modern_mouse_and_key_events_emit_base_signals() {
        let mut bw = make_base();
        let mouse_down = std::sync::Arc::new(std::sync::Mutex::new(None::<(Point, u32)>));
        let key_down = std::sync::Arc::new(std::sync::Mutex::new(None::<(u32, u32)>));
        let mouse_sink = mouse_down.clone();
        let key_sink = key_down.clone();
        bw.mouse_down.connect(move |args| {
            *mouse_sink.lock().unwrap() = Some(*args);
        });
        bw.key_down.connect(move |args| {
            *key_sink.lock().unwrap() = Some(*args);
        });

        bw.handle_event(&Event::MousePress { pos: Point::new(12, 34), button: 1 });
        bw.handle_event(&Event::KeyPress { key: 65, modifiers: 2 });

        assert_eq!(*mouse_down.lock().unwrap(), Some((Point::new(12, 34), 1)));
        assert_eq!(*key_down.lock().unwrap(), Some((65, 2)));
    }

    #[test]
    fn pointer_and_focus_events_emit_base_signals() {
        let mut bw = make_base();
        let hover = std::sync::Arc::new(std::sync::Mutex::new(None::<Point>));
        let released = std::sync::Arc::new(std::sync::Mutex::new(None::<(Point, u32)>));
        let focused = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let hover_sink = hover.clone();
        let release_sink = released.clone();
        let focus_sink = focused.clone();
        bw.hover.connect(move |pos| {
            *hover_sink.lock().unwrap() = Some(*pos);
        });
        bw.mouse_up.connect(move |args| {
            *release_sink.lock().unwrap() = Some(*args);
        });
        bw.focus_gained.connect(move || {
            focus_sink.store(true, std::sync::atomic::Ordering::SeqCst);
        });

        bw.handle_event(&Event::PointerMove {
            pos: Point::new(7, 9),
            pressure: 0.5,
            tilt_x: 0.0,
            tilt_y: 0.0,
        });
        bw.handle_event(&Event::PointerRelease {
            pos: Point::new(8, 10),
            button: 1,
            pressure: 0.0,
        });
        bw.handle_event(&Event::FocusGained { reason: FocusReason::Programmatic });

        assert_eq!(*hover.lock().unwrap(), Some(Point::new(7, 9)));
        assert_eq!(*released.lock().unwrap(), Some((Point::new(8, 10), 1)));
        assert!(focused.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn test_request_redraw_and_layout_emit_signals() {
        let bw = make_base();
        let redrawn = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let laid_out = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let r = redrawn.clone();
        let l = laid_out.clone();
        bw.redraw_requested.connect(move || r.store(true, std::sync::atomic::Ordering::SeqCst));
        bw.layout_requested.connect(move || l.store(true, std::sync::atomic::Ordering::SeqCst));

        bw.request_redraw();
        bw.request_layout();

        assert!(redrawn.load(std::sync::atomic::Ordering::SeqCst));
        assert!(laid_out.load(std::sync::atomic::Ordering::SeqCst));
    }

    /// The base records hover from the pair the runtime synthesises.
    ///
    /// This is the供述 side of the state channel: before it existed, a control that
    /// did not track hover itself reported `Normal` no matter what the theme said.
    #[test]
    fn base_records_hover_from_enter_and_leave() {
        let mut bw = make_base();
        assert!(!bw.is_hovered());
        bw.handle_event(&Event::MouseEnter { pos: Point::new(1, 1) });
        assert!(bw.is_hovered(), "MouseEnter sets hovered");
        bw.handle_event(&Event::MouseLeave { pos: Point::new(1, 1) });
        assert!(!bw.is_hovered(), "MouseLeave clears hovered");
    }

    /// Press takes both the `pressed` paint flag and the grab; release drops both.
    #[test]
    fn base_records_press_and_release_with_the_grab() {
        let mut bw = make_base();
        // A press inside the widget's rectangle arms the gesture.
        bw.handle_event(&Event::MousePress { pos: Point::new(50, 30), button: 1 });
        assert!(bw.is_pressed());
        assert!(bw.is_grabbed());
        bw.handle_event(&Event::MouseRelease { pos: Point::new(50, 30), button: 1 });
        assert!(!bw.is_pressed());
        assert!(!bw.is_grabbed());
    }

    /// A press outside the rectangle must not arm the gesture.
    ///
    /// A themed control can paint outside its box; without the hit test its state
    /// channel would report pressed for a click that landed elsewhere.
    #[test]
    fn base_ignores_a_press_outside_its_rectangle() {
        let mut bw = make_base();
        bw.handle_event(&Event::MousePress { pos: Point::new(400, 400), button: 1 });
        assert!(!bw.is_pressed());
        assert!(!bw.is_grabbed());
    }

    /// `pressed` is continuous while the grab is not: a drag off clears the paint but
    /// keeps the ownership, and a drag back restores it.
    #[test]
    fn base_keeps_the_grab_while_a_drag_leaves_and_returns() {
        let mut bw = make_base();
        bw.handle_event(&Event::MousePress { pos: Point::new(50, 30), button: 1 });
        bw.handle_event(&Event::MouseMove { pos: Point::new(400, 400) });
        assert!(!bw.is_pressed(), "dragging off stops painting pressed");
        assert!(bw.is_grabbed(), "but the gesture is still ours");
        bw.handle_event(&Event::MouseMove { pos: Point::new(50, 30) });
        assert!(bw.is_pressed(), "dragging back re-commits the paint");
    }

    /// A focus ring is painted only for the reasons that warrant one.
    #[test]
    fn base_focus_reason_decides_the_ring() {
        let mut bw = make_base();
        assert!(!bw.draws_focus_ring(), "no focus, no ring");
        bw.handle_event(&Event::FocusGained { reason: FocusReason::Pointer });
        assert!(bw.focus_reason().is_some(), "focused");
        assert!(!bw.draws_focus_ring(), "a pointer press must not paint a ring");
        bw.handle_event(&Event::FocusGained { reason: FocusReason::Tab });
        assert!(bw.draws_focus_ring(), "Tab paints a ring");
        bw.handle_event(&Event::FocusLost);
        assert!(bw.focus_reason().is_none(), "FocusLost clears the reason");
    }
}
